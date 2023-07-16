# SolHedge
## A protocol that brings cost-efficient and easy to use p2p options to the Solana blockchain

Solhedge brings options for hedging to the Solana blockchain
with a very low cost to users and high efficiency.

Users can sell Put Options or Call Options by creating what we call
a Vault Factory, where it is specified the maturity (time when the 
option will expire, all options are European style), the strike,
the base asset and the quote asset.

In this initial MVP implementation, only Put Options are being implemented.
Also, only options for wBTC (Wormhole) quoted in USDC are supported (all in localnet,
with mock assets)

A Vault Factory may contain multiple Vaults. A vault adds the lot size
for that vault (for instance, a vault may be of lots of $10^{-3}$ wBTC) and 
is associated to the option sellers (makers) as well as the option buyers 
(takers) for that vault. Each vault support up to $2^{16}$ makers and $2^{16}$ takers.
When a user enters in a vault (as a maker or taker) only the accounts to support
her entrance are created by her, so the transaction costs are very low for everyone.

When a user enters a put option vault as a maker, this means that (at this moment) she
wants to sell an option taker the right to sell her wBTC at strike price at a given time in the future. Suppose
that June 16th 2023 is a date in the future.
To sell this put option, she have to deposit in the vault the corresponding value (in USDC) at her entrance.
For instance, if the vault lot size is 0.001 bitcoins and she wants to sell put options for
0.1 bitcoin (100 vault lots) at 25000 USDC at maturity time, June 16th 2023, she must at the entrance
in the vault deposit $0.1*25000 = 2500$ USDC. This may be interesting to this maker because
she even wanted to buy and hold bitcoin now at a higher price (let's say 27000 USDC), so it will be ok
to buy later at 25000 and still get a premium for selling this option.

Not let's say a taker wants to buy the previous option, because he is holding bitcoin 
and he is afraid that bitcoin may be below 25000 dollars by June 16th 2023.
He is willing to pay a premium now to have the right to sell 0.1 bitcoin for 2500 USDC by June 16th.
As this is a right, it will only be fullfilled in favor of the option buyer. That is, if by June 16th
bitcoin is above 25000 USDC, the option will not be exercised, and both users get their original deposit back.

However, if by June 16th 2023 bitcoin is below 25000 dollars, the option taker has the interest of selling it
at this strike price. So in the option settlement, the maker will get the 0.1 bitcoin and the taker will get the 2500 dollars.

As an option is a derivative of the subjacent asset, the price (premium) it is worth is a function of the subjacent asset and
its volatility. We consider that for casual users it is too hard to determine a fair price to a option, and 
a order book would probably have many unrealistic expectations, stalling the market, in the abscence of professional market
makers. As we wanted Solhedge to work in this context, with only end users with limited financial knowledge and without the
help of professional book makers, we devise a methodology that the system automatically computes the fair price (premium) of
an option. Therefore, options are always sold at this automatic fair price.

When a user wants to buy an option, he asks an oracle to update the current fair price for that option, that is written in the
blockchain using a method only available to the oracle (`oracle_update_price`). As the oracle has to pay for the transaction, the user also send a small tip to the oracle at this time, to cover this fee. If the user is ok with the price, he sends a buy
order, with the fair price he saw and a slippage amount to the system. The system compares if the current fair price (written by the oracle in the blockchain) is within the slippage tolerance (plus fair price, both passed by the user) and buys the option to the user. 
Notice that the system does not have to trust any data passed by the user, if the user passes a fair price that is far from the 
fair price in the blockchain written by the oracle, the transaction will fail. But this guarantees that the user knows the premium
he will pay. 

As we said above, in order to calculate the fair premium price for an option, we have an oracle, that will be a server-side 
node js app that can receive requests from the user and communicate with the blockchain. For the moment, we only simulate
the oracle in the tests. (file `oracle.ts`). The oracle uses *Hello Moon* API to get the prices for the tokens, that will
be used to compute fair prices. To this end, both current price and historical prices are used, as we apply the classic Black-Scholes 
formulas (see e.g. https://www.macroption.com/black-scholes/) with historical volatility, using a window of the same size as the future window from now to the moment of the option maturity. The oracle automatically switches between ONE_HOUR, FIVE_MIN and ONE_MIN granularity (depending in how much in the future is the option maturity) in order to assure sufficient samples for the statistical calculations.

At this moment, we limit options to at most 30 days in the future, as in this case we use a 30 day window to compute historical
volatility, and this was the limit of historical data for Hello Moon API. Also, option vault factories are frozen 30 minutes before maturity. When a option vault factory is frozen, it is no longer possible to buy or sell options in its vaults. Only after maturity
the options may be settled. 

This is a work in progress. All the functionalities have not yet been completed. I already implemented:
- Creation of Put Vault Factories and Vaults.
- Users may enter put option vaults as makers and/or takers.
- Put option makers can adjust their positions (increase by depositing more USDC, or decrease, if their offer has not been already sold).
- Put option takers can buy and fund the puts options they bought with base assets up to the limit they bought the right to sell at any
time before the options are freezed (currently 30 minutos before maturity).
- Takers can ask the oracle to update fair price for put option.
- Oracle computes fair price for put option using Black Scholes formulas backed by Hello Moon data and updates the blockchain.
- After maturity, takers and makers can ask the oracle to settle the option price at the vault factory.
- After an option is settled, makers and takers can exit the option, with the option being exercised if it was favorable
  to the takers.
- An emergency exit mode was implemented. If more than a grace period has passed (currently 15 days) and the option settle price
has not been updated in the blockchain (remember, any taker and/or maker can ask the oracle to settle the option at any time after its 
maturity), we assume that the world outside the blockchain has collapsed and both takers and makers can take their deposited
assets back (as if the option should not been exercised).

You can run a simulation of the implemented code by running `anchor test`. You will need to define a `.env` with a `HELLO_MOON_BEARER` variable with your API key, as in the `.env.example`.

You can see an execution of `anchor test` at this link: https://youtu.be/SLUw9Yh3vig

## Devnet development

All the developments listed above were done and tested on localnet. There we used the real mint addresses of USDC and wBTC (wormhole),
by deploying the files `usdc-mock.json` and `wbtc-mock.json` in our local test validator (see `Anchor.toml`). 
The next step was to migrate and test on devnet. This is being done at the branch [`devnet-devel`](https://github.com/srmq/anchor-solhedge/tree/devnet-devel). As I could no longer mint USDC and wBTC on devnet, I coded a faucet of
fake USDC and wBTC (see program `snake-minter-devnet`), that mints [SnakeDollar](https://solscan.io/token/BJvndCYS1eMf1bg6vyJCjZiUEFcnZ5DeZKJiyZCjwN6K?cluster=devnet) and [SnakeBTC](https://solscan.io/token/6p728Y98qrSrvjRQmmvRLqa3JJ4P9RyLwbJ42DHxG7tP?cluster=devnet), that will be used in 
devnet as mock replacements for USDC and wBTC (tests on `snake-minter-devnet.ts`). The code that creates the token is at the directory `snake-tokens`.

## Economics

We plan to take a percentage of the option premium as protocol fees (say, 1%). Notice that users get the full amount of the option,
only the premium pays a fee. We plan to share the fee with front-ends. Let's say, we keep 50% and give 50% for the frontend. For this
end we receive a frontend wallet in the option buying method (see function `taker_buy_lots_put_option_vault`). This currently is  even permissionless, and anyone could implement a frontend
for the protocol.

## About the author

Sergio has about 20 years of programming experience. He has a doctorate degree in computer science, in the field of Artificial Intelligence (Paris 6 University, France). He is also an university professor and a certified financial advisor in Brazil.

