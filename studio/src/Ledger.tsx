import type { AccountData, QuantityData, ViewId } from "./api";
import { IdLink } from "./Inspectors";
import { Amount } from "./Quantity";

/** One asset of an account: what it holds now, and what it held before where that differs. */
export type LedgerRow = {
  asset: ViewId;
  /** What it holds now; none once the asset is gone. */
  now?: string;
  /** What it held before, where that differs from now. */
  before?: string;
  /** Whether the balance changes anywhere in the timeline. */
  changes: boolean;
};

/**
 * An account's balances against what it held before: every asset either
 * holds, those it no longer holds included, in the order the account holds
 * them. `changing`, the assets that change somewhere in the timeline, folds
 * the rest away.
 */
export function ledgerRows(account: AccountData | undefined, before: AccountData | undefined, changing?: ReadonlySet<string> | null): LedgerRow[] {
  const prior = new Map((before?.balances ?? []).map((balance) => [balance.asset.key, balance.quantity]));
  const rows: LedgerRow[] = (account?.balances ?? []).map((balance) => {
    const was = before ? prior.get(balance.asset.key) ?? "0" : balance.quantity;
    return { asset: balance.asset, now: balance.quantity, before: was !== balance.quantity ? was : undefined, changes: !changing || changing.has(balance.asset.key) };
  });
  const held = new Set(rows.map((row) => row.asset.key));
  for (const gone of before?.balances ?? []) {
    if (!held.has(gone.asset.key)) rows.push({ asset: gone.asset, before: gone.quantity, changes: true });
  }
  return rows;
}

/**
 * An account's ledger: each balance, before → after where it changed, the
 * balances that never change in the timeline counted and folded.
 */
export function AccountView({ account, before, changing }: { account?: AccountData; before?: AccountData; changing?: ReadonlySet<string> | null }) {
  if (!account && !before) return <p className="muted">Not here</p>;
  const rows = ledgerRows(account, before, changing);
  // Nothing of it changes in the timeline: nothing to fold it behind.
  const folds = rows.some((row) => row.changes);
  const live = folds ? rows.filter((row) => row.changes) : rows;
  const still = folds ? rows.filter((row) => !row.changes) : [];
  return <div className="ledger">
    {!account && <p className="muted">Closed</p>}
    {live.map((row) => <Balance key={row.asset.key} row={row} />)}
    {still.length > 0 && <details className="unchanged-assets">
      <summary>{still.length} unchanged</summary>
      {still.map((row) => <Balance key={row.asset.key} row={row} />)}
    </details>}
    {rows.length === 0 && <p className="muted">No balances</p>}
  </div>;
}

function Balance({ row }: { row: LedgerRow }) {
  const changed = row.before !== undefined;
  return <div className={`balance${changed ? " changed" : ""}${row.now === undefined ? " ghost" : ""}`}>
    <span><IdLink subject={{ kind: "asset", id: row.asset }} /></span>
    {changed && <small><Amount asset={row.asset.key} atoms={row.before!} /> →</small>}
    <strong><Amount asset={row.asset.key} atoms={row.now ?? "0"} /></strong>
  </div>;
}

/**
 * The accounts at a step, one row each. After a step, those it changed come
 * first, each with the balances it changed, before → after; the rest are
 * counted and folded. At the start, every account.
 */
export function StateList({ accounts, before, focus }: { accounts: AccountData[]; before?: AccountData[]; focus?: string }) {
  const prior = new Map((before ?? []).map((account) => [account.account.key, account]));
  const rows = accounts.map((account) => ({
    account,
    changes: before ? ledgerRows(account, prior.get(account.account.key) ?? { account: account.account, balances: [] }).filter((row) => row.before !== undefined) : [],
  }));
  const item = ({ account, changes }: (typeof rows)[number]) => <li key={account.account.key} className={`${changes.length > 0 ? "changed" : ""}${focus === account.account.key ? " focused" : ""}`}>
    <div className="state-account">
      <IdLink subject={{ kind: "account", id: account.account }} />
      {changes.length > 0 && <small>{changes.length} changed</small>}
    </div>
    {changes.length > 0 && <div className="ledger state-changes">{changes.map((row) => <Balance key={row.asset.key} row={row} />)}</div>}
  </li>;
  if (!before) return <ul className="state-list">{rows.map(item)}</ul>;
  const changed = rows.filter((row) => row.changes.length > 0);
  const still = rows.filter((row) => row.changes.length === 0);
  return <div className="state">
    {changed.length > 0 && <ul className="state-list">{changed.map(item)}</ul>}
    {still.length > 0 && <details className="unchanged-accounts" open={changed.length === 0 || still.some((row) => row.account.account.key === focus)}>
      <summary>{still.length} unchanged</summary>
      <ul className="state-list">{still.map(item)}</ul>
    </details>}
  </div>;
}

/** Who holds an asset at a step: each account and how much, before → after where it changed. */
export function AssetView({ asset, holders }: { asset: ViewId; holders: { account: ViewId; now?: QuantityData["quantity"]; before?: QuantityData["quantity"] }[] }) {
  if (holders.length === 0) return <p className="muted">No account holds it</p>;
  return <div className="ledger">
    {holders.map((holder) => {
      const changed = holder.before !== undefined && holder.before !== holder.now;
      return <div key={holder.account.key} className={`balance${changed ? " changed" : ""}${holder.now === undefined ? " ghost" : ""}`}>
        <span><IdLink subject={{ kind: "account", id: holder.account }} /></span>
        {changed && <small><Amount asset={asset.key} atoms={holder.before!} /> →</small>}
        <strong><Amount asset={asset.key} atoms={holder.now ?? "0"} /></strong>
      </div>;
    })}
  </div>;
}
