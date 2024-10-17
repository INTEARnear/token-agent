# Tokens & Staking agent

1 binary, 2 agents, because why not. Sends the right `.well-known/ai-plugin.json` based on Host header.

# Tokens Agent:

- Get account's total balance, including FTs, their amounts, amount in USD, NEAR, staking information, etc. (powered by [FastNear API](https://github.com/fastnear/fastnear-api-server-rs))
- Get prices of tokens (powered by [prices.intear.tech](https://prices.intear.tech))

# Staking Agent:

- Get account's staking information
- Stake NEAR on a specific pool
- Unstake NEAR from a specific pool
- Withdraw unstaked NEAR from a specific pool
- Withdraw or unstake a specific amount of NEAR from any pool, or multiple pools, prioritizing withdrawable NEAR
