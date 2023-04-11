import cx from "classnames";
import { useUser, useSignIn, useClerk } from "@clerk/nextjs";
import Image from "next/image";
import { Inter } from "next/font/google";

import styles from "./Header.module.scss";
const inter = Inter({ subsets: ["latin"], weight: ["700", "400"] });

export default function Header() {
  const { signIn, isLoaded } = useSignIn();
  const { user } = useUser();
  const { signOut } = useClerk();

  const signInWithGitHub = () => {
    if (isLoaded) {
      signIn.authenticateWithRedirect({
        strategy: "oauth_github",
        redirectUrl: "/sso-callback",
        redirectUrlComplete: "/",
      });
    }
  };

  return (
    <header className={styles.header}>
      <h1 className={cx(inter.className, styles.title)}>Trunk</h1>
      {user ? (
        <button onClick={() => signOut()} className={styles.loginButton}>
          <Image src="/images/github.svg" alt="GitHub logo" width={20} height={20}></Image>
          <span className={cx(inter.className, styles.authText, styles.userName)}>{user.fullName}</span>
          <span className={cx(inter.className, styles.logout)}>Logout</span>
        </button>
      ) : (
        <button onClick={() => signInWithGitHub()} className={styles.loginButton}>
          <Image src="/images/github.svg" alt="GitHub logo" width={20} height={20}></Image>
          <span className={cx(inter.className, styles.authText)}>Sign in with GitHub</span>
        </button>
      )}
    </header>
  );
}
