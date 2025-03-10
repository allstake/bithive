import { tGas } from "near-workspaces";
import readline from "readline";

export const envBuilder = {
  choices: ["stg", "prod", "testnet3", "testnet4", "signet"],
  describe: "Environment name",
  demandOption: true,
};

export async function confirmAction(message: string) {
  const rl = readline.createInterface({
    input: process.stdin,
    output: process.stdout,
  });
  console.log(message);
  return new Promise((resolve) => {
    rl.question(`Continue ?`, resolve);
  });
}

export function nearTGas(gas: number) {
  return BigInt(tGas(gas));
}
