# Hippolyzer Second Life Login Process

The login process for a Second Life client like Hippolyzer is a multi-stage process that begins with an HTTP-based authentication and culminates in a UDP-based session with a simulator. This document details that process with a focus on network destinations and packet contents, as implemented in `hippolyzer/lib/client/hippo_client.py`.

## Phase 1: HTTP XML-RPC Authentication

The very first step is to authenticate with the Linden Lab login server. This is not done over the game protocol (UDP) but via an XML-RPC call over HTTPS.

### Destination Address/Port

-   **URL**: `https://login.agni.lindenlab.com/cgi-bin/login.cgi`
-   **Protocol**: HTTPS (TCP Port 443)

### Packet Content

The client constructs a detailed payload dictionary containing user credentials and system information. This dictionary is then serialized into an XML-RPC request.

**Code Example: Constructing the Login Payload**
```python
# From: hippolyzer.lib.client.hippo_client.py -> HippoClient.login

split_username = username.split(" ")
if len(split_username) < 2:
    first_name = split_username[0]
    last_name = "Resident"
else:
    first_name, last_name = split_username

payload = {
    "address_size": 64,
    "agree_to_tos": int(agree_to_tos),
    "channel": "Hippolyzer",
    "extended_errors": 1,
    "first": first_name,
    "last": last_name,
    "host_id": "",
    "id0": hashlib.md5(str(self._mac).encode("ascii")).hexdigest(),
    "mac": hashlib.md5(str(self._mac).encode("ascii")).hexdigest(),
    "mfa_hash": "",
    "passwd": "$1$" + hashlib.md5(str(password).encode("ascii")).hexdigest(),
    "platform": "lnx",
    "platform_string": "Linux 6.6",
    "platform_version": "2.38.0",
    "read_critical": 0,
    "start": str(start_location), # e.g., "last", "home", or "uri:Region&128&128&128"
    "token": "",
    "version": version("hippolyzer"),
    "options": list(self._options),
}

# This payload is then sent via an HTTP POST request.
async with self.http_session.post(
        login_uri,
        data=xmlrpc.client.dumps((payload,), "login_to_simulator"),
        headers={"Content-Type": "text/xml", "User-Agent": self.settings.USER_AGENT},
        ssl=self.settings.SSL_VERIFY,
) as resp:
    # ... process response
```

The server validates this information and, if successful, returns another XML-RPC response. This response is critical as it contains the session tokens and the address of the simulator (region) the client should connect to. Key fields in the response include:

-   `agent_id`, `session_id`, `secure_session_id`: Unique identifiers for this session.
-   `circuit_code`: A unique code to identify this UDP connection.
-   `sim_ip`, `sim_port`: The **destination IP address and port** for the next phase.
-   `seed_capability`: A URL for a web-based API to get more capabilities (endpoints) for the session.

## Phase 2: UDP Simulator Connection and Handshake

With the session information from the login server, the client can now connect to the simulator. This phase uses the UDP protocol, which is how most real-time game data is transmitted.

### Network Path: Local UDP Socket Setup

Before sending anything, the client prepares to receive UDP traffic from the simulator. It creates a UDP socket bound to `0.0.0.0` and a random, OS-assigned port.

**Code Example: Creating the UDP Transport**
```python
# From: hippolyzer.lib.client.hippo_client.py -> HippoClient._create_transport

async def _create_transport(self) -> Tuple[AbstractUDPTransport, HippoClientProtocol]:
    loop = asyncio.get_event_loop_policy().get_event_loop()
    transport, protocol = await loop.create_datagram_endpoint(
        lambda: HippoClientProtocol(self.session),
        local_addr=('0.0.0.0', 0)) # Binds to all interfaces, random port
    transport = SocketUDPTransport(transport)
    return transport, protocol
```
This is a crucial step. If a firewall or restrictive NAT prevents incoming UDP packets from reaching this socket, the login will fail.

### Destination Address/Port

-   **IP Address**: The `sim_ip` from the Phase 1 response.
-   **Port**: The `sim_port` from the Phase 1 response.

### Packet Content: The Handshake Process

The client and server exchange a series of UDP packets to establish a valid, secure session.

**Step 1: `UseCircuitCode`**

This is the first UDP packet the client sends to the simulator. Its purpose is to link the unauthenticated UDP packet to the authenticated HTTP session from Phase 1.

**Code Example: Sending `UseCircuitCode`**
```python
# From: hippolyzer.lib.client.hippo_client.py -> HippoClientRegion.connect

await self.circuit.send_reliable(
    Message(
        "UseCircuitCode",
        Block(
            "CircuitCode",
            Code=self.session().circuit_code,
            SessionID=self.session().id,
            ID=self.session().agent_id,
        ),
    )
)
```

**Step 2: `CompleteAgentMovement`**

For the initial login, the client must immediately tell the simulator it has "arrived".

**Code Example: Sending `CompleteAgentMovement`**
```python
# From: hippolyzer.lib.client.hippo_client.py -> HippoClientRegion.connect

if main_region:
    await self.complete_agent_movement()

# Where complete_agent_movement is:
async def complete_agent_movement(self) -> None:
    await self.circuit.send_reliable(
        Message(
            "CompleteAgentMovement",
            Block(
                "AgentData",
                AgentID=self.session().agent_id,
                SessionID=self.session().id,
                CircuitCode=self.session().circuit_code
            ),
        )
    )
```

**Step 3: Waiting for `RegionHandshake`**

The client now listens for a `RegionHandshake` packet from the server. If this packet never arrives, it's almost always due to a network path issue.

**Step 4: Sending `RegionHandshakeReply` and `AgentThrottle`**

Once the `RegionHandshake` is received, the client is considered partially connected. It must reply to finalize the connection and also send its bandwidth throttle settings.

**Code Example: Finalizing the Handshake**
```python
# From: hippolyzer.lib.client.hippo_client.py -> HippoClientRegion.connect

# First, wait for the server's handshake
self.name = str((await region_handshake_fut)['RegionInfo'][0]['SimName'])

# Then, send our reply
await self.circuit.send_reliable(
    Message(
        "RegionHandshakeReply",
        Block("AgentData", AgentID=self.session().agent_id, SessionID=self.session().id),
        Block(
            "RegionInfo",
            Flags=(
                RegionHandshakeReplyFlags.SUPPORTS_SELF_APPEARANCE
                | RegionHandshakeReplyFlags.VOCACHE_CULLING_ENABLED
            )
        )
    )
)

# And send our throttle settings
await self.circuit.send_reliable(
    Message(
        "AgentThrottle",
        # ... throttle data ...
    )
)
```

After these steps, the UDP connection is fully established. The client also starts polling a web-based "Event Queue" (using the `seed_capability` URL) to receive messages that are not sent over UDP.

## Network Path and Firewall/NAT Considerations

1.  **Outbound Traffic**: The client must be able to send TCP packets to `login.agni.lindenlab.com` on port 443 and UDP packets to the simulator's IP/port. This is usually allowed by default on most networks.

2.  **Inbound Traffic**: This is the most common point of failure. The simulator responds to the client's public IP and the source port of the outbound UDP packets.
    -   **NAT (Network Address Translation)**: Most routers perform NAT. When the client sends a UDP packet, the router creates a temporary entry in its state table mapping `[Client's_Internal_IP]:[Client's_Port]` to `[Router's_Public_IP]:[A_New_Port]`. The simulator sends its response to the router's public IP and port. The router then uses its state table to forward the packet to the correct internal client.
    -   **Firewalls**: A personal or network firewall could block the incoming UDP response from the simulator. You must ensure that established UDP responses are allowed.
    -   **Troubleshooting**: If the client sends `UseCircuitCode` but never receives `RegionHandshake`, the problem lies here. The return packets are being dropped. Testing with a public UDP echo server can confirm if your network path allows for UDP request/response cycles at all. If that works but Second Life doesn't, the issue might be a firewall specifically targeting game protocols or the high port numbers used by simulators.