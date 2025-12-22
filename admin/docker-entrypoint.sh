#!/bin/sh
# Generate config.js from environment variable
if [ -n "$API_URL" ]; then
    echo "const API_CONFIG = { API_URL: '${API_URL}' };" > /usr/share/nginx/html/config.js
    echo "✅ Generated config.js with API_URL: ${API_URL}"
else
    echo "⚠️ API_URL not set, using default"
    echo "const API_CONFIG = { API_URL: 'https://wg0qj9h1b7.execute-api.us-west-2.amazonaws.com/prod' };" > /usr/share/nginx/html/config.js
fi

# Execute the original nginx entrypoint with any passed arguments
exec /docker-entrypoint.sh "$@"

