import styles from "./Header.module.scss";
import cx from "classnames";

import { Inter } from "next/font/google";

const inter = Inter({ subsets: ["latin"], weight: ["700"] });
export default function Header() {
  return (
    <header className={styles.header}>
      <h1 className={cx(inter.className, styles.title)}>Trunk</h1>
      <p>Login</p>
    </header>
  );
}
