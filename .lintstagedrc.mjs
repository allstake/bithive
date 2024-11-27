export default {
  "contracts/**/*.rs": [
    "rustfmt"
  ],
  "*.ts?(x)": [
    () => "pnpm lint",
  ],
  "*.{js,jsx,ts,tsx}": [
    "prettier --write"
  ],
};
