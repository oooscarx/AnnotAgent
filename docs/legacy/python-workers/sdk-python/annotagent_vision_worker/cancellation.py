"""Cooperative cancellation scoped by inference request ID."""

from __future__ import annotations

from threading import Event, Lock


class CancellationRegistry:
    def __init__(self) -> None:
        self._events: dict[str, Event] = {}
        self._lock = Lock()

    def begin(self, request_id: str) -> Event:
        with self._lock:
            if request_id in self._events:
                raise ValueError("duplicate active request_id")
            event = Event()
            self._events[request_id] = event
            return event

    def cancel(self, request_id: str) -> bool:
        with self._lock:
            event = self._events.get(request_id)
            if event is None:
                return False
            event.set()
            return True

    def finish(self, request_id: str) -> None:
        with self._lock:
            self._events.pop(request_id, None)
