#!node_modules/.bin/ts-node

import yargs from "yargs";
import { hideBin } from "yargs/helpers";
import { deployBtcClient, deployBtcLightClient } from "./commands/deploy";
import { init } from "./commands/init";
import { submitDeposit } from "./commands/submit_deposit";
import { queueWithdraw } from "./commands/queue_withdraw";
import { submitWithdraw } from "./commands/submit_withdraw";

yargs(hideBin(process.argv))
  .strict()
  .help()
  .command(deployBtcClient)
  .command(deployBtcLightClient)
  .command(submitDeposit)
  .command(queueWithdraw)
  .command(submitWithdraw)
  .command(init)
  .parse();
