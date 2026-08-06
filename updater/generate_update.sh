#!/bin/bash

set -e

CYPRESS_ASTRO_DIR="$HOME/projects/cypress-astro"
FLUTTER_DIR="$CYPRESS_ASTRO_DIR/cypress-catalog/flutter"
DIST_DIR="$CYPRESS_ASTRO_DIR/dist"
UPDATE_DIR="/tmp/cypress-update-gen"

echo "Building Cypress Astro..."
cd "$CYPRESS_ASTRO_DIR"
./build.sh

echo "Building Cypress Catalog Flutter..."
cd "$FLUTTER_DIR"
./build.sh

echo "Preparing update package..."
rm -rf "$UPDATE_DIR"
mkdir -p "$UPDATE_DIR"

# Create apply_update.sh
cat << 'EOF' > "$UPDATE_DIR/apply_update.sh"
#!/bin/bash

set -e

# The working directory for this script will be /tmp/cypress-update
cd "$(dirname "$0")"

echo "Copying cypress-server..."
cp -f cypress-server /home/cedar/cedar/bin/

echo "Copying flutter build..."
rm -rf /home/cedar/cedar/cedar-aim/cedar_flutter/build
cp -r build /home/cedar/cedar/cedar-aim/cedar_flutter/

echo "Setting capabilities..."
sudo setcap "cap_sys_time,cap_dac_override,cap_chown,cap_fowner,cap_net_bind_service+ep" /home/cedar/cedar/bin/cypress-server

echo "Update applied successfully!"
EOF

chmod +x "$UPDATE_DIR/apply_update.sh"

# Copy artifacts
cp "$DIST_DIR/cypress-server" "$UPDATE_DIR/"
cp -r "$FLUTTER_DIR/build" "$UPDATE_DIR/"

# Zip everything
mkdir -p "$DIST_DIR"
cd "$UPDATE_DIR"
zip -qr "$DIST_DIR/cypress_update.zip" ./*

echo "Update zip generated at $DIST_DIR/cypress_update.zip"

# Clean up
rm -rf "$UPDATE_DIR"
