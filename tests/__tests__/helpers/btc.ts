import assert from "assert";
import * as bitcoin from "bitcoinjs-lib";
import bs58check from "bs58check";
import { ECPairInterface } from "ecpair";
import { ec as EC } from "elliptic";
import hash from "hash.js";
import { sha3_256 } from "js-sha3";
import { base_decode } from "near-api-js/lib/utils/serialize";

export function depositScriptV1(
  userPubkey: Buffer,
  allstakePubkey: Buffer,
  sequence: number,
): Buffer {
  return bitcoin.script.fromASM(
    `
    OP_IF
        ${bitcoin.script.number.encode(sequence).toString("hex")}
        OP_CHECKSEQUENCEVERIFY
        OP_DROP
        ${userPubkey.toString("hex")}
        OP_CHECKSIG
    OP_ELSE
        OP_2
        ${userPubkey.toString("hex")}
        ${allstakePubkey.toString("hex")}
        OP_2
        OP_CHECKMULTISIG
    OP_ENDIF
    `
      .trim()
      .replace(/\s+/g, " "),
  );
}

export function soloWithdrawScript(userSig: Buffer): Buffer {
  return bitcoin.script.compile([userSig, bitcoin.opcodes.OP_TRUE]);
}

export function multisigWithdrawScript(
  userSig: Buffer,
  allstakeSig: Buffer,
): Buffer {
  return bitcoin.script.compile([
    bitcoin.opcodes.OP_0,
    userSig,
    allstakeSig,
    bitcoin.opcodes.OP_FALSE,
  ]);
}

export function idToHash(txid: string): Buffer {
  return Buffer.from(txid, "hex").reverse();
}

export function toOutputScript(
  address: string,
  network: bitcoin.networks.Network,
): Buffer {
  return bitcoin.address.toOutputScript(address, network);
}

export function getWitnessUtxo(out: any): any {
  delete out.address;
  out.script = Buffer.from(out.script, "hex");
  return out;
}

export function reconstructSignature(big_r: string, big_s: string) {
  const r = big_r.slice(2).padStart(64, "0");
  const s = big_s.padStart(64, "0");

  const rawSignature = Buffer.from(r + s, "hex");

  if (rawSignature.length !== 64) {
    throw new Error("Invalid signature length.");
  }

  return rawSignature;
}

function najPublicKeyStrToUncompressedHexPoint() {
  const rootPublicKey =
    "secp256k1:4NfTiv3UsGahebgTaHyD9vF8KYKMBnfd6kh94mK6xv8fGBiJB8TBtFMP5WWXz6B89Ac1fbpzPwAvoyQebemHFwx3";
  const res =
    "04" +
    Buffer.from(base_decode(rootPublicKey.split(":")[1])).toString("hex");
  return res;
}

async function deriveChildPublicKey(
  parentUncompressedPublicKeyHex: any,
  signerId: any,
  path = "",
) {
  const ec = new EC("secp256k1");
  const scalarHex = sha3_256(
    `near-mpc-recovery v0.1.0 epsilon derivation:${signerId},${path}`,
  );

  const x = parentUncompressedPublicKeyHex.substring(2, 66);
  const y = parentUncompressedPublicKeyHex.substring(66);

  // Create a point object from X and Y coordinates
  const oldPublicKeyPoint = ec.curve.point(x, y);

  // Multiply the scalar by the generator point G
  const scalarTimesG = ec.g.mul(scalarHex);

  // Add the result to the old public key point
  const newPublicKeyPoint = oldPublicKeyPoint.add(scalarTimesG);
  const newX = newPublicKeyPoint.getX().toString("hex").padStart(64, "0");
  const newY = newPublicKeyPoint.getY().toString("hex").padStart(64, "0");
  return "04" + newX + newY;
}

export async function uncompressedHexPointToBtcAddress(
  publicKeyHex: string,
  network: string,
) {
  // Step 1: SHA-256 hashing of the public key
  const publicKeyBytes = Uint8Array.from(Buffer.from(publicKeyHex, "hex"));

  const sha256HashOutput = await crypto.subtle.digest(
    "SHA-256",
    publicKeyBytes,
  );

  // Step 2: RIPEMD-160 hashing on the result of SHA-256
  const ripemd160 = hash
    .ripemd160()
    .update(Buffer.from(sha256HashOutput))
    .digest();

  // Step 3: Adding network byte (0x00 for Bitcoin Mainnet)
  const network_byte = network === "bitcoin" ? 0x00 : 0x6f;
  const networkByte = Buffer.from([network_byte]);
  const networkByteAndRipemd160 = Buffer.concat([
    networkByte,
    Buffer.from(ripemd160),
  ]);

  // Step 4: Base58Check encoding
  const address = bs58check.encode(networkByteAndRipemd160);

  return address;
}

export async function deriveAddress(
  accountId: string,
  derivation_path: string,
  network: string,
) {
  const publicKey = await deriveChildPublicKey(
    najPublicKeyStrToUncompressedHexPoint(),
    accountId,
    derivation_path,
  );
  const address = await uncompressedHexPointToBtcAddress(publicKey, network);
  return { publicKey: Buffer.from(publicKey, "hex"), address };
}

export function compressPubKey(pubKeyBuffer: Buffer) {
  // Extract the x and y coordinates
  const xCoord = pubKeyBuffer.slice(1, 33); // First 32 bytes after the prefix
  const yCoord = pubKeyBuffer.slice(33, 65); // Next 32 bytes

  // Determine the prefix for the compressed key
  const yCoordIsEven = yCoord[yCoord.length - 1] % 2 === 0;
  const prefix = yCoordIsEven ? 0x02 : 0x03;

  // Create the compressed public key
  const compressedPubKeyBuffer = Buffer.concat([Buffer.from([prefix]), xCoord]);

  // Convert the compressed public key to a hex string
  return compressedPubKeyBuffer;
}

type PartialSignResult = {
  partialSignedPsbt: bitcoin.Psbt;
  hashToSign: Buffer;
};

export function partialSignPsbt(
  psbt: bitcoin.Psbt,
  signer: ECPairInterface,
): PartialSignResult {
  let hashToSign: Buffer | null = null;
  const signFn = signer.sign;
  const sign = (hash: Buffer, lowR = false): Buffer => {
    hashToSign = hash;
    return signFn.call(signer, hash, lowR);
  };
  signer.sign = sign;
  const partialSignedPsbt = psbt.clone().signInput(0, signer);
  assert(!!hashToSign, "No hash to sign");
  return {
    partialSignedPsbt,
    hashToSign: hashToSign!,
  };
}
