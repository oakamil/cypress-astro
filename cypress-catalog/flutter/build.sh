#!/bin/bash

# Navigate to the script's directory (cypress-catalog)
cd "$(dirname "$0")"

# Remove the current output directory
rm -rf build

# Build the flutter web app
flutter build web --no-web-resources-cdn

# Fix font families so they can be found without the package prefix
sed -i 's/"family":"packages\/cedar_flutter\//"family":"/g' build/web/assets/FontManifest.json

# Move cedar_flutter assets to the root assets directory so they can be found
mkdir -p build/web/assets/assets
mv build/web/assets/packages/cedar_flutter/assets/* build/web/assets/assets/
