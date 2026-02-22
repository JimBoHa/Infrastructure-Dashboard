#!/bin/bash
# tools/create-adr.sh

ADR_DIR="docs/ADRs"
TEMPLATE="# {TITLE}

* **Status:** Proposed
* **Date:** $(date +%Y-%m-%d)

## Context
What is the issue that we're seeing that is motivating this decision or change?

## Decision
What is the change that we're proposing and/or doing?

## Consequences
What becomes easier or more difficult to do and any risks introduced by this change?"

# Ensure directory exists
mkdir -p "$ADR_DIR"

# Get the Title from arguments
if [ -z "$1" ]; then
  echo "Usage: $0 \"Title of the Decision\""
  exit 1
fi

TITLE="$1"
# Convert title to kebab-case for filename (e.g. "Use AI" -> "use-ai")
SLUG=$(echo "$TITLE" | tr '[:upper:]' '[:lower:]' | tr ' ' '-')

# Find the next number
LAST_NUM=$(ls "$ADR_DIR" | grep -E '^[0-9]{4}-' | sort | tail -n 1 | cut -d'-' -f1)
if [ -z "$LAST_NUM" ]; then
    NEXT_NUM="0001"
else
    NEXT_NUM=$(printf "%04d" $((10#$LAST_NUM + 1)))
fi

FILENAME="${ADR_DIR}/${NEXT_NUM}-${SLUG}.md"

# Create the file with the template
echo "$TEMPLATE" | sed "s/{TITLE}/$NEXT_NUM. $TITLE/" > "$FILENAME"

echo "Created new ADR: $FILENAME"
