from __future__ import annotations

import asyncio
import ipaddress
import socket
from urllib.parse import SplitResult, urlsplit


class UnsafeDestinationError(ValueError):
    pass


def parse_public_https_url(url: str) -> SplitResult:
    parsed = urlsplit(url)
    host = (parsed.hostname or "").rstrip(".").lower()
    if (
        parsed.scheme != "https"
        or not host
        or not parsed.netloc
        or parsed.username
        or parsed.password
        or parsed.query
        or parsed.fragment
    ):
        raise UnsafeDestinationError("External provider URLs must be HTTPS without credentials")
    if host in {"localhost", "metadata", "metadata.google.internal"} or host.endswith(
        (".localhost", ".local", ".internal", ".test", ".invalid")
    ):
        raise UnsafeDestinationError("Local and internal destinations are not allowed")
    try:
        address = ipaddress.ip_address(host)
    except ValueError:
        return parsed
    if not address.is_global:
        raise UnsafeDestinationError("Non-public IP destinations are not allowed")
    return parsed


async def ensure_public_destination(url: str) -> None:
    parsed = parse_public_https_url(url)
    host = parsed.hostname
    if host is None:
        raise UnsafeDestinationError("Destination hostname is missing")
    try:
        ipaddress.ip_address(host)
        return
    except ValueError:
        pass
    port = parsed.port or 443
    loop = asyncio.get_running_loop()
    try:
        addresses = await loop.getaddrinfo(host, port, type=socket.SOCK_STREAM)
    except OSError as exc:
        raise UnsafeDestinationError("Provider hostname could not be resolved") from exc
    resolved = {ipaddress.ip_address(item[4][0]) for item in addresses}
    if not resolved or any(not address.is_global for address in resolved):
        raise UnsafeDestinationError("Provider hostname resolves to a non-public IP")
