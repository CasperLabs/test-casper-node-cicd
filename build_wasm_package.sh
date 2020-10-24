#!/bin/bash
set -ue

abspath() {
  # generate absolute path from relative path
  # $1     : relative filename
  # return : absolute path
  if [ -d "$1" ]; then
    # dir
    (cd "$1"; pwd)
  elif [ -f "$1" ]; then
    # file
    if [[ $1 == */* ]]; then
      echo "$(cd "${1%/*}"; pwd)/${1##*/}"
    else
      echo "$(pwd)/$1"
    fi
  fi
}


export RUN_DIR=$(dirname $(abspath $0))
NODE_CONFIG_FILE="$RUN_DIR/node/Cargo.toml"
export WASM_PACKAGE_VERSION="$(grep -oP "^version\s=\s\"\K(.*)\"" $NODE_CONFIG_FILE | sed -e s'/"//g')"
export CL_WASM_DIR="$RUN_DIR/target/wasm32-unknown-unknown/release"
export CL_OUTPUT_S3_DIR="$RUN_DIR/s3_artifacts/${WASM_PACKAGE_VERSION}"

if [ ! -d $CL_OUTPUT_S3_DIR ]; then
  mkdir -p "${CL_OUTPUT_S3_DIR}"
fi

# package all wasm files
echo "[INFO] Checking if wasm files are ready under the path $CL_WASM_DIR"
if [ -d "$CL_WASM_DIR" ]; then
  ls -al $CL_WASM_DIR/*wasm
  echo "[INFO] Creating a tar.gz pacakge: $CL_WASM_DIR"
  pushd $CL_WASM_DIR
  tar zcvf $CL_OUTPUT_S3_DIR/casper-contracts.tar.gz *wasm
  popd
else
  echo "[ERROR] No wasm dir: $CL_WASM_DIR"
  exit 1
fi
