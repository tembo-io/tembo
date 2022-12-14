import pandas as pd

import matplotlib.style
import matplotlib as mpl
import matplotlib.pyplot as plt

mpl.style.use('ggplot')

def plot_response(filename: str):
    df = pd.read_csv(filename)
    df.query("Type == 'GET'", inplace=True)
    df.columns = [x.lower() for x in df.columns]
    rps = df["requests/s"].astype(int).mean()
    error = df["failure count"].astype(int).mean() / df["request count"].astype(int).mean()
    df[['90%', '95%', '98%', '99%', '99.9%']].plot(
        kind="bar",
        title=f"Response Time Percentiles(ms)\n @ RPS={rps}, err_rate={error:.2%}\n {filename}")


def combine_plt(files: list[str]):
    all_df = []
    for p in files:
        _df = pd.read_csv(p)
        _df["filename"] = p.replace("_stats.csv", "")
        _df.query("Type == 'GET'", inplace=True)
        _df.columns = [x.lower() for x in _df.columns]
        all_df.append(_df)
    df = pd.concat(all_df)
    df[["filename", '98%', '99%', '99.9%']].plot(
        kind="bar",
        title=f"Response Time Percentiles(ms)",
        x="filename",
        y=['98%', '99%', '99.9%'],
        rot=0,
        xlabel="webserver"
    )

    plt.savefig('./benchmark.png')

    return df



if __name__ == "__main__":
    combine_plt(
        [
            # "stand-alone-actix_stats.csv",
            # "pgext-TcpListener_stats.csv",
            "fastapi_stats.csv"
        ])
    