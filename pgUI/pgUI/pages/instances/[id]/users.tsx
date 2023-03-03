import React from 'react';
import { useRouter } from 'next/router';
import Head from 'next/head';

import Header from '../../../components/Header';
import Main from '../../../components/Main';

const MonitoringPage = () => {
  const router = useRouter();
  const { id } = router.query;

  return (
    <>
      <Head>
        <title>CoreDB</title>
        <meta name="description" content="Users" />
        <link rel="icon" href="favicon.ico" />
      </Head>
      <Header userName="Rico Suave" />
      <Main hasLeftBar>
        <p>Users: {id}</p>
      </Main>
    </>
  );
};

export default MonitoringPage;
