import cx from "classnames";
import { Inter } from "next/font/google";
import Image from "next/image";

import styles from "./Footer.module.scss";

import Logo from "/public/images/Logo.svg";
import CoreDB from "/public/images/CoreDB.svg";
const inter = Inter({ subsets: ["latin"], weight: ["400"] });

export default function Footer() {
  return (
    <footer>
      <a href="https://coredb.io/" className={styles.footer}>
        <p className={cx(inter.className, styles.text)}>Sponsored by</p>
        <Image className={styles.image} src={Logo} alt="Logo" />
        <Image className={styles.image} src={CoreDB} alt="Logo" />
      </a>
    </footer>
  );
}
