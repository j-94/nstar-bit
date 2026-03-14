"""
N★ Bit graph health monitor.

The WebSocket endpoint sends graph health snapshots in response to any message.
The monitor pings every `ping_interval` seconds to keep receiving updates.

This is a monitor, not a command channel.
"""
from __future__ import annotations

import asyncio
import json
from typing import Callable, Optional

import websockets
from websockets.exceptions import ConnectionClosed

from .types import GraphHealth


class Monitor:
    def __init__(
        self,
        ws_url: str,
        on_update: Callable[[GraphHealth], None],
        ping_interval: float = 5.0,
    ) -> None:
        self._ws_url = ws_url
        self._on_update = on_update
        self._ping_interval = ping_interval
        self._task: Optional[asyncio.Task] = None  # type: ignore[type-arg]
        self._stopped = False

    async def start(self) -> "Monitor":
        """Start the monitor. Connects and begins receiving health snapshots."""
        self._stopped = False
        self._task = asyncio.create_task(self._run())
        return self

    async def stop(self) -> None:
        """Stop the monitor and close the WebSocket connection."""
        self._stopped = True
        if self._task:
            self._task.cancel()
            try:
                await self._task
            except asyncio.CancelledError:
                pass
            self._task = None

    async def _run(self) -> None:
        while not self._stopped:
            try:
                async with websockets.connect(self._ws_url) as ws:
                    # Send initial ping to receive first snapshot
                    await ws.send("ping")
                    while not self._stopped:
                        try:
                            msg = await asyncio.wait_for(
                                ws.recv(), timeout=self._ping_interval + 1
                            )
                            try:
                                health = GraphHealth.from_dict(json.loads(msg))
                                self._on_update(health)
                            except (json.JSONDecodeError, KeyError):
                                pass  # Unparseable frame — ignore
                        except asyncio.TimeoutError:
                            # No message received — ping to get a fresh snapshot
                            await ws.send("ping")
            except ConnectionClosed:
                if not self._stopped:
                    await asyncio.sleep(2.0)
            except OSError:
                if not self._stopped:
                    await asyncio.sleep(2.0)
