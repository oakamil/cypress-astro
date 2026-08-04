#!/bin/bash

# Navigate to the script's directory (cypress-catalog)
cd "$(dirname "$0")"

# Build the flutter web app
flutter build web --no-web-resources-cdn
