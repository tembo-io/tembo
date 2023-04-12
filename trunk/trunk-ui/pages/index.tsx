import Head from "next/head";
import Image from "next/image";
import { Inter } from "next/font/google";
import styles from "./index.module.scss";

import Header from "@/components/Header";
import Hero from "@/components/Hero";
import Footer from "@/components/Footer";

export default function Home() {
  return (
    <>
      <Head>
        <title>Trunk</title>
        <meta name="description" content="Trunk" />
        <meta name="viewport" content="width=device-width, initial-scale=1" />
        {/* TODO: Add favicon */}
        {/* <link rel="icon" href="/favicon.ico" /> */}
      </Head>
      <main className={styles.main}>
        <Header />
        <Hero />
        <Footer />
      </main>
    </>
  );
}
