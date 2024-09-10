import { CommandModule } from "yargs";
import { confirmAction, envBuilder } from "../helper";
import { initNear } from "../near";
import fs from "fs";
import { getConfig } from "../config";

interface Args {
  env: string;
}

export const deployBtcClient: CommandModule<unknown, Args> = {
  command: "deploy",
  describe: "Deploy and init BTC client contract",
  builder: {
    env: envBuilder,
  },
  async handler({ env }) {
    const config = await getConfig(env);
    await deployContract(
      env,
      "res/btc_client.wasm",
      config.accountIds.btcClient,
    );
    process.exit();
  },
};

export const deployBtcLightClient: CommandModule<unknown, Args> = {
  command: "deploy-btc-lc",
  describe: "Deploy mocked BTC light client",
  builder: {
    env: envBuilder,
  },
  async handler({ env }) {
    const config = await getConfig(env);
    await deployContract(
      env,
      "res/mock_btc_lightclient.wasm",
      config.accountIds.btcLightClient,
    );
    process.exit();
  },
};

async function deployContract(
  env: string,
  codePath: string,
  accountId: string,
) {
  const { signer } = await initNear(env, accountId);
  const code = fs.readFileSync(codePath);
  await confirmAction(`Deploy contract to ${accountId}`);
  await signer.deployContract(code);
  console.log("Contract deployed");
}
