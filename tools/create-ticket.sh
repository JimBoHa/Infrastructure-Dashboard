#!/bin/bash
# tools/create-ticket.sh

TICKET_DIR="project_management/tickets"
TEMPLATE="# {TITLE}

**Status:** Open

## Description
(Paste the detailed description here)

## Scope
* [ ] Item 1
* [ ] Item 2

## Acceptance Criteria
* [ ] Criteria 1

## Notes
"

# Ensure directory exists
mkdir -p "$TICKET_DIR"

# Get the Title from arguments
if [ -z "$1" ]; then
  echo "Usage: $0 \"Title of the Ticket\""
  exit 1
fi

TITLE="$1"
SLUG=$(echo "$TITLE" | tr '[:upper:]' '[:lower:]' | tr ' ' '-')

# Find the next number (matches TICKET-XXXX format)
LAST_NUM=$(ls "$TICKET_DIR" | grep -E '^TICKET-[0-9]{4}-' | sort | tail -n 1 | cut -d'-' -f2)
if [ -z "$LAST_NUM" ]; then
    NEXT_NUM="0001"
else
    NEXT_NUM=$(printf "%04d" $((10#$LAST_NUM + 1)))
fi

FILENAME="${TICKET_DIR}/TICKET-${NEXT_NUM}-${SLUG}.md"

# Create the file with the template
echo "$TEMPLATE" | sed "s/{TITLE}/TICKET-$NEXT_NUM: $TITLE/" > "$FILENAME"

echo "Created new Ticket: $FILENAME"
