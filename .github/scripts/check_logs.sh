#!/bin/bash
tail -n 100 cubby_output.log
if grep -q "panic" cubby_output.log; then
  echo "CLI crashed"
  exit 1
fi
if ! grep -q "Server listening on 0.0.0.0:3030" cubby_output.log; then
  echo "Server did not start correctly"
  exit 1
fi
if grep -q "No windows found" cubby_output.log; then
  echo "No windows were detected"
  exit 1
fi
if grep -q "tesseract not found" cubby_output.log; then
  echo "Tesseract OCR not found"
  exit 1
fi
echo "CLI ran successfully without crashing"
