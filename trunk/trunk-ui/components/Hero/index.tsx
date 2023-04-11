import cx from "classnames";
import styles from "./Hero.module.scss";

import { Inter } from "next/font/google";

const inter = Inter({ subsets: ["latin"], weight: ["400"] });

export default function Hero() {
  return (
    <section>
      <h1 className={cx(inter.className, styles.title)}>Expand Your Postgres Capabilities</h1>
      <h2 className={cx(inter.className, styles.subtitle)}>The easiest way to publish and install PostgreSQL extensions. </h2>
      <p className={cx(inter.className, styles.body)}>
        Trunk is an open-source package installer and registry for PostgreSQL extensions. Use the Trunk CLI to easily publish and install
        PostgreSQL extensions and their dependencies.
      </p>
    </section>
  );
}
