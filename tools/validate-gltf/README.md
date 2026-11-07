# glTF checks

```sh
cd tools/validate-gltf && npm install
node validate.mjs /path/a.glb /path/b.glb           # Khronos validator: errors / warnings
cp /path/a.glb . && node threejs-shot.cjs "a.glb,a.glb:bend" out.png   # three.js render
```

`threejs-shot.cjs` needs Playwright with Chromium (`npm install playwright && npx playwright install chromium`, or a preinstalled one). It serves this folder on port 8765 and renders each listed
file side by side with headless Chromium (Playwright). `:bend` rotates the
upper arm, forearm and thigh to check skinning; `:morph0` sets the first
morph weight to 0. Files must be copied into this folder (they are git-ignored).
