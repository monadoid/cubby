CURRENT_RELEASE=$1

CHANGELOG_PUBLIC_PATH=cubby-app-tauri/public/CHANGELOG.md

LAST_CHANGELOG=$(awk '{printf "%s\\n", $0}' content/changelogs/v0.1.98.md | sed 's/"/\\"/g')

# Note: CrabNebula integration disabled - no longer using their service
# Get last release from git tags instead
LAST_TAG=$(git describe --tags --abbrev=0 2>/dev/null || echo "")

if [ -z "$LAST_TAG" ]; then
  echo "No previous tags found, using first commit"
  COMMIT_LAST_RELEASE=$(git rev-list --max-parents=0 HEAD)
else
  COMMIT_LAST_RELEASE=$(git rev-parse $LAST_TAG)
fi

COMMIT_CURRENT_RELEASE=$(git log -1 --format="%H")
COMMIT_CURRENT_RELEASE=${2:-$COMMIT_CURRENT_RELEASE}

if [ "$COMMIT_LAST_RELEASE" == "" ]; then
  echo "Failed to get the commit hash for the last release"
  echo "CHANGELOG_GENERATED=0" >> $GITHUB_ENV
  exit 1
fi

if [ "$COMMIT_CURRENT_RELEASE" == "" ]; then
  echo "Failed to get the commit hash for the current release"
  echo "CHANGELOG_GENERATED=0" >> $GITHUB_ENV
  exit 1
fi

# If both are equal, then there's nothing to add to the changelog
if [ "$COMMIT_LAST_RELEASE" == "$COMMIT_CURRENT_RELEASE" ]; then
  echo "No new commits to add to the changelog"
  echo "CHANGELOG_GENERATED=0" >> $GITHUB_ENV
  exit 0
fi

COMMITS=$(git log --oneline $COMMIT_LAST_RELEASE..$COMMIT_CURRENT_RELEASE --oneline | tr '\n' ', ' | sed 's/"/\\"/g')

# Debug: Print the commits being sent
echo "DEBUG: Commits being sent to OpenAI:"
echo "$COMMITS"
echo "---"

CONTENT=$(
  curl https://api.openai.com/v1/chat/completions \
    -H "Content-Type: application/json" \
    -H "Authorization: Bearer $OPENAI_API_KEY" \
    -d "{
      \"model\": \"gpt-4o-mini\",
      \"messages\": [
        {
          \"role\": \"system\",
          \"content\": \"You are a helpful assistant.\nThe user is using a product called "cubby" which records his screen and mics 24/7. The user ask you questions and you use his cubby recordings to answer him.\nYou will generate a changelog for the new cubby update based on a list of commits.\nHere are a some guidelines for your responses:\n- only adds to the changelog what brings clear customer value\n- categorize the changes into 'New Features', 'Improvements' and 'Fixes'. Anything not matching these guidelines should not be included on your response\n- Deploys, merges, and software maintenance tasks which does not bring clear value to the end-user should not be included.\n\nUse the following changelog file as an example: $LAST_CHANGELOG\"
        },
        {
          \"role\": \"user\",
          \"content\": \"Here are my commits: $COMMITS\"
        }
      ]
    }"
)

# Debug: Print the raw response from OpenAI
echo "DEBUG: Raw OpenAI response:"
echo "$CONTENT"
echo "---"

CONTENT=$(jq '.choices[0].message.content' <<< $CONTENT)

# exit if the content is null
if [ "$CONTENT" == "null" ]; then
  echo "Failed to generate changelog content."
  echo "CHANGELOG_GENERATED=0" >> $GITHUB_ENV
  exit 1
fi

# Create directory content/changelogs if it doesn't exist
mkdir -p content/changelogs

# Create a new file with the current release as the name
echo -e ${CONTENT//\"/} > content/changelogs/$CURRENT_RELEASE.md
SHORT_COMMIT_LAST_RELEASE=$(echo $COMMIT_LAST_RELEASE | cut -c 1-5)
SHORT_COMMIT_CURRENT_RELEASE=$(echo $COMMIT_CURRENT_RELEASE | cut -c 1-5)

# Add the full changelog on the end of the file
echo """
#### **Full Changelog:** [$SHORT_COMMIT_LAST_RELEASE..$SHORT_COMMIT_CURRENT_RELEASE](https://github.com/monadoid/cubby/compare/$SHORT_COMMIT_LAST_RELEASE..$SHORT_COMMIT_CURRENT_RELEASE)
""" >> content/changelogs/$CURRENT_RELEASE.md

# Copy the new changelog to the main changelog file
cp content/changelogs/$CURRENT_RELEASE.md $CHANGELOG_PUBLIC_PATH

# Output the current release version to be used in the workflow
echo "CURRENT_RELEASE=$CURRENT_RELEASE" >> $GITHUB_ENV

# Set the flag to indicate that the changelog was generated
echo "CHANGELOG_GENERATED=1" >> $GITHUB_ENV
