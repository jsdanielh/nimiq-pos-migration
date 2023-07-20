# Nimiq Proof-of-Stake Migration

This repository contains a set of tools and utilities to help the migration process from Nimiq Proof-of-Work to Nimiq 
Proof-of-Stake (PoS).
The intended users of these utilities are nodes who want to become validators in the Nimiq PoS chain.
The functionality in this repository includes:

- Nimiq Proof-of-Stake Genesis builder: This tool is used to create the Nimiq POS Genesis block, which will include the
  account state (balances) from the current Nimiq POW Chain. It is worth to note that this block would be a continuation
  of the Nimiq POW chain, with the addition of the first validator set and the Nimiq POW balances, i.e.: this block
  would constitute the first election block of the Nimiq PoS chain.
- Validator readiness: Sends a validator ready transaction to signal that the validator is ready to start the migration
  process and also monitors the PoW chain for other validators readiness signal. When enough validators are ready, then
  an election block candidate is automatically elected to be used as the final PoW state to be migrated.
- Nimiq PoS Wrapper: Starts the Nimiq PoS client when the minimum number of block confirmations for the election
  candidate is satisfied.
- Nimiq PoS history builder: Migrates the Nimiq PoW history into Nimiq PoS. This is only necessary for history nodes who
  want to include and support the full history of the Nimiq PoW and PoS chain.
- Nimiq PoS state builder: Migrates the Nimiq PoW state into Nimiq PoS.