#!/usr/bin/env bash

set -e

export BENCH_DIR=./bench
export DATA_DIR="${BENCH_DIR}/lcwa_gov_image_data"
export ZIPFILE="${BENCH_DIR}/lcwa_gov_image_data.zip"
export ZIPURL="https://lclabspublicdata.s3.us-east-2.amazonaws.com/lcwa_gov_image_data.zip"

if [ ! -d "${DATA_DIR}" ]; then
  mkdir -p "${DATA_DIR}"
fi

if [ ! -d "${DATA_DIR/data}" ]; then

  if [ ! -f "${ZIPFILE}" ]; then
    curl -o "${ZIPFILE}" "${ZIPURL}"
  fi

  if [ ! -d "${DATA_DIR}/data" ]; then
    unzip -d "${DATA_DIR}" "${ZIPFILE}"
  fi
fi


