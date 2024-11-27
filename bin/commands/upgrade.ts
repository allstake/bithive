import { envBuilder } from "../helper";
import { getConfig } from "../config";
import { CommandModule } from "yargs";
import { initNear } from "../near";

interface Args {
  env: string;
}

export const upgrade: CommandModule<unknown, Args> = {
  command: "upgrade",
  describe: "Call upgrade on BTC client contract",
  builder: {
    env: envBuilder,
  },
  async handler({ env }) {
    const config = await getConfig(env);
    const { signer } = await initNear(env, config.accountIds.owner);
    await signer.functionCall({
      contractId: config.accountIds.btcClient,
      methodName: "upgrade",
      attachedDeposit: BigInt(1),
    });
  },
};
