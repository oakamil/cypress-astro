#!/bin/bash
UPDATE_ZIP="$HOME/run/cypress_update.zip"
UPDATE_DIR="/tmp/cypress-update"

if [ -f "$UPDATE_ZIP" ]; then
    echo "Staged update found at $UPDATE_ZIP"

    # Empty out /tmp/cypress-update
    rm -rf "$UPDATE_DIR"
    mkdir -p "$UPDATE_DIR"

    # Unzip there
    unzip -q "$UPDATE_ZIP" -d "$UPDATE_DIR"

    # Run the apply script with sudo
    if [ -f "$UPDATE_DIR/apply_update.sh" ]; then
        chmod +x "$UPDATE_DIR/apply_update.sh"
        sudo "$UPDATE_DIR/apply_update.sh"
    else
        echo "Error: apply_update.sh not found in the update zip"
    fi

    # Delete staged zip
    rm -f "$UPDATE_ZIP"

    echo "Update finished."
else
    echo "No update staged at $UPDATE_ZIP"
fi
