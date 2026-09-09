"""UIAPI-002 socket regressions; no workspace, provider, or fixed service ports."""
import errno
import socket
import unittest

from http_fixture import free_port


class FixturePortTests(unittest.TestCase):
    def test_user_service_port_is_forbidden(self):
        with self.assertRaises(ValueError):
            free_port(8787)

    def test_active_listener_is_not_shared(self):
        with socket.socket() as listener:
            listener.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
            listener.bind(("127.0.0.1", 0))
            listener.listen(1)
            with self.assertRaises(OSError) as error:
                free_port(listener.getsockname()[1])
            self.assertEqual(error.exception.errno, errno.EADDRINUSE)

    def test_time_wait_without_listener_can_restart_immediately(self):
        with socket.socket() as listener:
            listener.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
            listener.bind(("127.0.0.1", 0))
            listener.listen(1)
            port = listener.getsockname()[1]
            with socket.create_connection(("127.0.0.1", port), timeout=2) as client:
                accepted, _ = listener.accept()
                with accepted:
                    # Server actively closes, placing its port into TIME_WAIT.
                    accepted.shutdown(socket.SHUT_WR)
                    self.assertEqual(client.recv(1), b"")
                    client.shutdown(socket.SHUT_WR)
                    self.assertEqual(accepted.recv(1), b"")
        # Reproduce the original check failure with no listener still open.
        with socket.socket() as old_probe:
            with self.assertRaises(OSError) as error:
                old_probe.bind(("127.0.0.1", port))
            self.assertEqual(error.exception.errno, errno.EADDRINUSE)
        self.assertEqual(free_port(port), port)
        # The actual listener strategy must also work after the probe closes.
        with socket.socket() as restarted:
            restarted.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
            restarted.bind(("127.0.0.1", port))
            restarted.listen(1)
            with socket.create_connection(("127.0.0.1", port), timeout=2):
                accepted, _ = restarted.accept()
                accepted.close()


if __name__ == "__main__":
    unittest.main(verbosity=2)
