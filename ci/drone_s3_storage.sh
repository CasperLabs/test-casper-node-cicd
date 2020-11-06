#!/usr/bin/env bash

set -ex

# This script allows uploading, downloading and purging of files to s3 for sharing between drone pipelines.

# Making unique string for temp folder name in S3
# Adding DRONE_REPO to DRONE_BUILD_NUMBER, because build is only unique per repo.
# replacing the / in DRONE_REPO name with _ to not be path in S3
DRONE_UNIQUE="${DRONE_BUILD_NUMBER}_${DRONE_REPO/\//_}"

valid_commands=("put" "get" "del")
ACTION=$1
if [[ " ${valid_commands[*]} " != *" $ACTION "* ]]; then
  echo "Invalid command passed: $ACTION"
  echo "Possible commands are:"
  echo " put <local source with ending /> <s3 target>"
  echo " get <s3 source with ending /> <local target>"
  ecbo " del "
  exit 1
fi

if [[ "$ACTION" != "del" ]]; then
  SOURCE=$2
  TARGET=$3

  if [ -z "$SOURCE" ]; then
    echo "Source not provided"
    exit 1
  fi

  if [ -z "$TARGET" ]; then
    echo "Target not provided"
    exit 1
  fi
fi

CL_S3_BUCKET='casperlabs-cicd-artifacts'
CL_S3_LOCATION="drone_temp/${DRONE_UNIQUE}"

#echo "-H \"X-Vault-Token: $CL_VAULT_TOKEN\"" > ~/.curlrc

# get aws credentials files
CREDENTIAL_FILE_TMP="$RUN_DIR/s3_vault_output.json"
TOKEN="-H \"X-Vault-Token: $CL_VAULT_TOKEN\""
CL_VAULT_URL="${CL_VAULT_HOST}/v1/sre/cicd/s3/aws_credentials"
CREDENTIALS=$(curl -s -q "$TOKEN" -X GET "$CL_VAULT_URL")
if [ ! -f "$CREDENTIALS" ]; then
  echo "[ERROR] Unable to fetch aws credentials from vault: $CL_VAULT_URL"
  exit 1
else
  echo "[INFO] Found credentials"
  echo "[DEBUG] $CREDENTIALS"
  # get just the body required by bintray, strip off vault payload
  AWS_ACCESS_KEY_ID=$(echo "$CREDENTIALS" | jq -r .data.cicd_agent_to_s3.aws_access_key)
  export AWS_ACCESS_KEY_ID
  AWS_SECRET_ACCESS_KEY=$(echo "$CREDENTIAL_FILE_TMP" | jq -r .data.cicd_agent_to_s3.aws_secret_key)
  export AWS_SECRET_ACCESS_KEY
  echo "AWS ACCESS : $AWS_ACCESS_KEY_ID"
  echo "AWS ACCESS : $AWS_SECRET_ACCESS_KEY"
fi

exit
case "$ACTION" in
  "put")
    echo "sync ${SOURCE} s3://${CL_S3_BUCKET}/${CL_S3_LOCATION}/${TARGET}"
    s3cmd sync "${SOURCE}" "s3://${CL_S3_BUCKET}/${CL_S3_LOCATION}/${TARGET}"
    ;;
  "get")
    echo "sync s3://${CL_S3_BUCKET}/${CL_S3_LOCATION}/${SOURCE} ${TARGET}"
    s3cmd sync "s3://${CL_S3_BUCKET}/${CL_S3_LOCATION}/${SOURCE}" "${TARGET}"
    ;;
  "del")
    echo "del --recursive s3://${CL_S3_BUCKET}/${CL_S3_LOCATION}"
    s3cmd del --recursive "s3://${CL_S3_BUCKET}/${CL_S3_LOCATION}"
    ;;
esac
