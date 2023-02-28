import React from 'react';
import { useRouter } from 'next/router';
import Head from 'next/head';

import Header from '../../components/Header';
import Main from '../../components/Main';

const Instance = () => {
  const router = useRouter();
  const { id } = router.query;

  return (
    <>
      <Head>
        <title>CoreDB</title>
        <meta name="description" content="Welcome to CoreDB" />
        <link rel="icon" href="favicon.ico" />
      </Head>
      <Header userName="Rico Suave" />
      <Main hasLeftBar>
        <p>Instance: {id}</p>
      </Main>
    </>
  );
};

export default Instance;
