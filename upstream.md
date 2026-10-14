# 本家リポジトリの更新を取り込む

このリポジトリは [anyrun-org/anyrun](https://github.com/anyrun-org/anyrun) の fork です。

| remote | URL |
|---|---|
| `origin` | `git@github.com:tetutaro/anyrun.git`（fork） |
| `upstream` | `https://github.com/anyrun-org/anyrun.git`（本家） |

## 取り込み手順

```sh
git fetch upstream && git merge upstream/master
```

取り込んだ結果を fork に反映する場合は、続けて以下を実行します。

```sh
git push origin master
```
