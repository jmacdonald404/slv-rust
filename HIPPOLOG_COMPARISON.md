# Hippolog Analysis Tool Guide

> **Note:** If you are reading this, refactoring probably just happened and it's necessary to review `slv.hippolog`, `oa.hippolog`, and `log.txt`. `slv.hippolog` contains the hippolyzer packet logs from the last run. `oa.hippolog` is a reference run of the official Second Life viewer through hippolyzer and should be adhered to strictly and maximally in order to handle the protocols properly. `log.txt` contains the Rust logs from the last run of SLV and helps track down which parts of our logic deviate from official behavior.

## Overview

The `target/debug/hippolog_analyzer` tool analyzes and compares packet logs between different Second Life viewer implementations to identify protocol compliance issues and implementation gaps.

## File Structure

- **`.hippolog files`** - Packet logs captured through the proxy from viewer implementations
- **`log.txt`** - Runtime logs that provide context for packet analysis

## Command Structure

```bash
target/debug/hippolog_analyzer [.hippolog file] [command] [options]
```

## Available Commands

### 1. `stats` (default)
Shows basic statistics about the hippolog file:
```bash
target/debug/hippolog_analyzer slv.hippolog stats
target/debug/hippolog_analyzer oa.hippolog stats
```

**Output includes:**
- Total entries count
- HTTP entries count  
- LLUDP entries count
- EQ entries count
- Region breakdown

### 2. `grep <pattern> [--case-sensitive]`
Searches for a string pattern in the logs and returns matching packets:
```bash
target/debug/hippolog_analyzer slv.hippolog grep "ViewerEffect"
target/debug/hippolog_analyzer oa.hippolog grep "OUT" --case-sensitive
```

**Features:**
- Case-insensitive search by default
- Returns list of matching packets with entry numbers
- Shows match locations within each packet
- Single match shows full packet details automatically

### 3. `detail <entry_index> [--pretty] [--decode-bytes]`
Views full packet details for a specific entry:
```bash
target/debug/hippolog_analyzer slv.hippolog detail 3 --pretty --decode-bytes
target/debug/hippolog_analyzer oa.hippolog detail 17 --pretty
```

**Options:**
- `--pretty` - Makes output more readable with formatting
- `--decode-bytes` - Provides additional bytes analysis

### 4. `list` - Enhanced Packet Overview
Shows all packet headers with message names for easy identification:
```bash
target/debug/hippolog_analyzer slv.hippolog list
target/debug/hippolog_analyzer oa.hippolog list | grep ViewerEffect
target/debug/hippolog_analyzer oa.hippolog list | grep AgentUpdate
```

**Output format:** `entry_number: [TYPE] METHOD - (MESSAGE_NAME) summary...`

**Example output:**
```
7: [LLUDP] OUT - (UseCircuitCode) Code=539246843, SessionID=00eb1392-8…
8: [LLUDP] OUT - (CompleteAgentMovement) CircuitCode=539246843
23: [LLUDP] OUT - (ViewerEffect) ID=fab4a502-a…, Type=9, Duration=0.5...
44: [LLUDP] OUT - (AgentUpdate) BodyRotation=<-0.0, 0.0…, HeadRotation=<0.0, 0.0,…
```

### 5. `flags` - Enhanced Packet Flags Analysis
Shows detailed packet flags, frequency, and trust level information:
```bash
target/debug/hippolog_analyzer slv.hippolog flags
target/debug/hippolog_analyzer oa.hippolog flags
```

**Output format:** `entry: [TYPE] Direction [Flags] Frequency/Trust #ID - (MessageName) Summary`

**Flag Legend:**
- `R` = Reliable (requires ACK)
- `RS` = Resent packet  
- `Z` = Zero-coded
- `A` = Appended ACKs

**Example output:**
```
18: [LLUDP] OUT [R] Fixed/NotTrusted #1 - (UseCircuitCode) Code=535237060, SessionID=...
19: [LLUDP] IN [-] Fixed/NotTrusted #1 - (PacketAck) ID=1
24: [LLUDP] IN [R|Z] Low/Trusted #2 - (AgentDataUpdate) FirstName=Fresh..., LastName=...
```

**Summary Statistics:**
- Reliable packet counts and percentages  
- Frequency distribution (High/Medium/Low/Fixed)
- Trust level distribution (Trusted/NotTrusted)
- Flag usage statistics

### 6. Other Commands
- `http` - Show HTTP entries only
- `lludp` - Show LLUDP entries only  
- `eq` - Show EQ entries only
- `http-summary` - Show HTTP request summaries (method, URL, status)
- `export` - Export back to hippolog format

## Comparison Workflow

### Step 1: Basic Statistics Comparison
```bash
target/debug/hippolog_analyzer implementation.hippolog stats
target/debug/hippolog_analyzer baseline.hippolog stats
```

Compare:
- Total packet counts
- Protocol distribution (HTTP vs LLUDP vs EQ)
- Region activity patterns

### Step 2: Find Missing Message Types
```bash
# Use enhanced list command to quickly identify message types
target/debug/hippolog_analyzer implementation.hippolog list | grep "(ViewerEffect)"
target/debug/hippolog_analyzer baseline.hippolog list | grep "(ViewerEffect)"

# Search for outgoing messages in both logs
target/debug/hippolog_analyzer implementation.hippolog grep "OUT"
target/debug/hippolog_analyzer baseline.hippolog grep "OUT" 

# Search for specific message types (alternative method)
target/debug/hippolog_analyzer baseline.hippolog grep "ViewerEffect"
target/debug/hippolog_analyzer implementation.hippolog grep "ViewerEffect"
```

### Step 3: Detailed Packet Analysis
```bash
# Examine specific packets that differ
target/debug/hippolog_analyzer baseline.hippolog detail 22 --pretty --decode-bytes
target/debug/hippolog_analyzer implementation.hippolog detail 3 --pretty --decode-bytes
```

### Step 4: Implementation Gap Analysis
Review `log.txt` for:
- Runtime context and error messages
- Protocol compliance information
- Implementation notes


## Common Comparison Patterns

### Finding Next Implementation Target
1. Compare stats to identify volume differences
2. Use grep to find message types we're missing
3. Use detail to examine packet structure
4. Check log.txt for context on previous attempts

### Example Analysis Session
```bash
# Basic comparison
target/debug/hippolog_analyzer implementation.hippolog stats
target/debug/hippolog_analyzer baseline.hippolog stats

# Comprehensive packet flags analysis
target/debug/hippolog_analyzer baseline.hippolog flags | head -30
target/debug/hippolog_analyzer implementation.hippolog flags | head -30

# Find reliable packets
target/debug/hippolog_analyzer baseline.hippolog flags | grep "\[R"
target/debug/hippolog_analyzer implementation.hippolog flags | grep "\[R"

# Enhanced packet overview with flags and trust levels
target/debug/hippolog_analyzer baseline.hippolog list | head -25

# Compare specific message types between implementations
target/debug/hippolog_analyzer baseline.hippolog list | grep "(ViewerEffect)"
target/debug/hippolog_analyzer implementation.hippolog list | grep "(ViewerEffect)"

# Find specific message types
target/debug/hippolog_analyzer baseline.hippolog list | grep "(MessageType)"
target/debug/hippolog_analyzer implementation.hippolog list | grep "(MessageType)"

# Check for problematic message types (from console parsing errors)
target/debug/hippolog_analyzer baseline.hippolog list | grep "(AgentUpdate)"
target/debug/hippolog_analyzer implementation.hippolog list | grep "(AgentUpdate)"

# HTTP request analysis
target/debug/hippolog_analyzer baseline.hippolog http-summary | head -20

# Detailed packet analysis with frequency/trust/flags
target/debug/hippolog_analyzer baseline.hippolog detail 19 --pretty
target/debug/hippolog_analyzer implementation.hippolog detail 8 --pretty

# Check outgoing messages
target/debug/hippolog_analyzer implementation.hippolog lludp
```

## Integration with Log Analysis

Always factor in `log.txt` when comparing:
- Contains runtime information from our implementation
- Shows what messages we attempted to send
- Provides context for why certain packets might be missing
- Records any errors or implementation notes

## Output Interpretation

### Stats Output Example
```
Hippolog Statistics:
  Total entries: 85
  HTTP entries: 3  
  LLUDP entries: 82
  EQ entries: 0
  Regions:
    Oritz: 82
```

### Grep Output Shows
- Entry number for use with `detail` command
- Packet type [HTTP/LLUDP/EQ]
- Method (IN/OUT)
- Summary of packet contents
- Match locations within packet

### Detail Output Provides
- Complete packet metadata
- Full data payload
- Decoded bytes analysis
- Protocol-specific field breakdown

## Troubleshooting Message Template Issues

When you see console errors like:
```
Left 413 bytes unread past end of ViewerEffect message, is your message template up to date?
Left 16 bytes unread past end of AgentUpdate message, is your message template up to date?
```

### Quick Diagnosis Steps:
1. **Identify the problematic message type** from console logs
2. **Find examples in both logs**:
   ```bash
   target/debug/hippolog_analyzer oa.hippolog list | grep "(ViewerEffect)"
   target/debug/hippolog_analyzer slv.hippolog list | grep "(ViewerEffect)" 
   ```
3. **Compare packet structures**:
   ```bash
   target/debug/hippolog_analyzer oa.hippolog detail 23 --pretty --decode-bytes
   target/debug/hippolog_analyzer slv.hippolog detail 28 --pretty --decode-bytes
   ```
4. **Check message template definitions** in `message_template.msg`
5. **Update templates** if the protocol has evolved beyond your current definitions

### Common Issues:
- **ViewerEffect**: Template expects basic fields but actual messages contain extended data (413 extra bytes suggests significant missing fields)
- **AgentUpdate**: Template missing 16 bytes suggests 1-2 additional fields (likely new protocol additions)
- **AgentFOV**: Template missing 16 bytes (similar to AgentUpdate)
- **UUIDNameRequest**: Template missing 15 bytes (likely string length or padding issue)

### Resolution Process:
1. Compare with latest `external/master-message-template/message_template.msg`
2. Update local template file
3. Rebuild with `cargo build`
4. Test with updated templates

This systematic approach helps identify implementation gaps and guides the next development phase.

