#!/bin/bash

BASE_URL="http://localhost:3000/api"

echo "=== 1. Test List Skills (Global) ==="
curl -s "$BASE_URL/skills" | grep -q "Success" && echo "PASS" || echo "FAIL"

echo "=== 2. Test List Skills (Org: backnotprop) ==="
curl -s "$BASE_URL/skills?owner=backnotprop" | grep -q "backnotprop" && echo "PASS" || echo "FAIL"

echo "=== 3. Test List Skills (Repo: rg_history) ==="
curl -s "$BASE_URL/skills?owner=backnotprop&repo=rg_history" | grep -q "rg_history" && echo "PASS" || echo "FAIL"

echo "=== 4. Test Get Skill Detail ==="
curl -s "$BASE_URL/skills/backnotprop/rg_history/rg_history" | grep -q "rg_history" && echo "PASS" || echo "FAIL"

echo "=== 5. Test Get Skill Version ==="
# Need to fetch version first or assume one exists from previous output. 
# Using the version from previous curl output: 0.0.1769878459
VERSION="0.0.1769878459"
curl -s "$BASE_URL/skills/backnotprop/rg_history/rg_history/versions/$VERSION" | grep -q "$VERSION" && echo "PASS" || echo "FAIL"

echo "=== 6. Test 404 for Non-existent Skill ==="
curl -s "$BASE_URL/skills/fake/fake/fake" | grep -q "404" && echo "PASS" || echo "FAIL"

echo "=== End of Verification ==="
