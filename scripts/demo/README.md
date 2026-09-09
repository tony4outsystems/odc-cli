# Regenerating the README demo

`odc` is a fake binary (never calls the real API) that prints canned output
for the three commands in `run_demo.sh`. To regenerate `demo.svg` at the repo
root after changing the script:

```bash
brew install asciinema
npm install -g svg-term-cli   # or use npx

TERM=xterm-256color asciinema rec scripts/demo/demo.cast \
  --command "bash scripts/demo/run_demo.sh" \
  --output-format asciicast-v2 --overwrite

svg-term --in scripts/demo/demo.cast --out demo.svg --window
```
