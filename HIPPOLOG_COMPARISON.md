# Hippolog Comparison Guide

## Overview

When I say "compare the hippo logs", this refers to using the `target/debug/hippolog_analyzer` tool to analyze and compare packet logs between our custom implementation and the official Second Life viewer.

## File Structure

- **`slv.hippolog`** - Our latest packet logs captured through the proxy from our SLV implementation
- **`oa.hippolog`** - Official Second Life viewer packet logs for comparison baseline  
- **`log.txt`** - Previous run logs that should be factored into comparison analysis

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

### 5. Other Commands
- `http` - Show HTTP entries only
- `lludp` - Show LLUDP entries only  
- `eq` - Show EQ entries only
- `http-summary` - Show HTTP request summaries (method, URL, status)
- `export` - Export back to hippolog format

## Comparison Workflow

### Step 1: Basic Statistics Comparison
```bash
target/debug/hippolog_analyzer slv.hippolog stats
target/debug/hippolog_analyzer oa.hippolog stats
```

Compare:
- Total packet counts
- Protocol distribution (HTTP vs LLUDP vs EQ)
- Region activity patterns

### Step 2: Find Missing Message Types
```bash
# Use enhanced list command to quickly identify message types
target/debug/hippolog_analyzer slv.hippolog list | grep "(ViewerEffect)"
target/debug/hippolog_analyzer oa.hippolog list | grep "(ViewerEffect)"

# Search for outgoing messages in both logs
target/debug/hippolog_analyzer slv.hippolog grep "OUT"
target/debug/hippolog_analyzer oa.hippolog grep "OUT" 

# Search for specific message types (alternative method)
target/debug/hippolog_analyzer oa.hippolog grep "ViewerEffect"
target/debug/hippolog_analyzer slv.hippolog grep "ViewerEffect"
```

### Step 3: Detailed Packet Analysis
```bash
# Examine specific packets that differ
target/debug/hippolog_analyzer oa.hippolog detail 22 --pretty --decode-bytes
target/debug/hippolog_analyzer slv.hippolog detail 3 --pretty --decode-bytes
```

### Step 4: Implementation Gap Analysis
Review `log.txt` for:
- Previous implementation attempts
- Known issues or limitations
- Protocol compliance notes

## Common Comparison Patterns

### Finding Next Implementation Target
1. Compare stats to identify volume differences
2. Use grep to find message types we're missing
3. Use detail to examine packet structure
4. Check log.txt for context on previous attempts

### Example Analysis Session
```bash
# Basic comparison
target/debug/hippolog_analyzer slv.hippolog stats
target/debug/hippolog_analyzer oa.hippolog stats

# Quick packet overview with message names
target/debug/hippolog_analyzer oa.hippolog list | head -25

# Compare specific message types between implementations
target/debug/hippolog_analyzer oa.hippolog list | grep "(ViewerEffect)"
target/debug/hippolog_analyzer slv.hippolog list | grep "(ViewerEffect)"

# Check for problematic message types (from console parsing errors)
target/debug/hippolog_analyzer oa.hippolog list | grep "(AgentUpdate)"
target/debug/hippolog_analyzer slv.hippolog list | grep "(AgentUpdate)"

# HTTP request analysis
target/debug/hippolog_analyzer oa.hippolog http-summary | head -20

# Find what we're missing
target/debug/hippolog_analyzer oa.hippolog grep "Type=" --case-sensitive

# Examine specific missing packets
target/debug/hippolog_analyzer oa.hippolog detail 22 --pretty

# Check what we currently send
target/debug/hippolog_analyzer slv.hippolog lludp
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