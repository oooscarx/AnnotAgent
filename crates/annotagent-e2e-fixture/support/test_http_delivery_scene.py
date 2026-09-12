import unittest
import urllib.parse

from http_delivery_scene import delivery_processing_selection


class DeliverySceneScopeTests(unittest.TestCase):
    def test_processing_uses_server_owned_delivery_scope_without_limit(self):
        selection = delivery_processing_selection("draft", "sample")

        self.assertEqual(selection, {"draft_id": "draft", "sample_test_id": "sample"})
        self.assertEqual(
            urllib.parse.urlencode(selection),
            "draft_id=draft&sample_test_id=sample",
        )
        self.assertNotIn("limit", selection)


if __name__ == "__main__":
    unittest.main(verbosity=2)
