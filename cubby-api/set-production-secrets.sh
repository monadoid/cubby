#!/bin/bash

# Script to set production secrets for cubby-api Cloudflare Worker
# This reads from .dev.vars but you should replace with production values

echo "setting production secrets for cubby-api..."
echo ""
echo "⚠️  warning: this will use values from .dev.vars"
echo "make sure these are your production values, not test values!"
echo ""
read -p "continue? (y/n) " -n 1 -r
echo ""

if [[ ! $REPLY =~ ^[Yy]$ ]]
then
    echo "aborted."
    exit 1
fi

# Read each line from .dev.vars and set as secret
while IFS='=' read -r key value; do
    # Skip comments and empty lines
    if [[ -z "$key" || "$key" == \#* ]]; then
        continue
    fi
    
    # Skip TUNNEL_DOMAIN (it's already in wrangler.toml as a regular var)
    if [[ "$key" == "TUNNEL_DOMAIN" ]]; then
        echo "⏭️  skipping $key (already in wrangler.toml)"
        continue
    fi
    
    echo "📝 setting $key..."
    echo "$value" | wrangler secret put "$key"
    
    if [ $? -eq 0 ]; then
        echo "✅ $key set successfully"
    else
        echo "❌ failed to set $key"
    fi
    echo ""
done < .dev.vars

echo ""
echo "✅ all secrets set!"
echo ""
echo "run 'pnpm run deploy' to deploy and then test with:"
echo "curl https://api.cubby.sh/debug/env"

