# v0.3.0-alpha

## Phase 3: Stability and Testing


### **TODO: Authentication Flow Implementation**

- [x] Send XML-RPC login request with required parameters  
  _Includes: first, last, passwd (MD5), start, channel, version, platform, mac, id0, agree_to_tos, read_critical, viewer_digest, options, etc._
  
- [x] Parse XML-RPC login response and extract required fields  
  _Includes: agent_id, session_id, secure_session_id, sim_ip, sim_port, circuit_code, etc._
  
- [x] Establish UDP connection to simulator using sim_ip and sim_port

- [x] Send UseCircuitCode UDP message to simulator and handle ack

- [ ] Send CompleteAgentMovement UDP message to simulator

- [ ] Send AgentUpdate UDP message to simulator and handle ack

- [ ] Handle AgentMovementComplete UDP message from simulator

- [ ] Handle optional login response fields (inventory, buddy-list, etc.)

- [/] Handle Terms of Service (ToS) agreement and critical message flags  
  _Partially implemented: ToS fetch and display logic exists, but may need more integration._
