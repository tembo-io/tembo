import React from 'react';
import type { NextPage } from 'next';
import Head from 'next/head';
import Main from '../components/Main';
import Header from '../components/Header';
import SearchBar from '../components/SearchBar';
import InstanceCard from '../components/InstanceCard';
import IconButton from '../components/IconButton';

const Home: NextPage = () => {
  return (
    <>
      <Head>
        <title>CoreDB</title>
        <meta name="description" content="Welcome to CoreDB" />
        <link rel="icon" href="favicon.ico" />
      </Head>
      <Header userName="Rico Suave" />
      <Main>
        <SearchBar placeholder="Search Instances" />
        <div
          style={{ display: 'grid', gridTemplateColumns: '1fr 1fr 1fr 1fr' }}
        >
          <InstanceCard
            dbName="CoreDB"
            id="1"
            paths
            properties={{
              connection: '',
              cpu: 1,
              dbname: 'My Cool Db',
              environment: 'test',
              memory: '16 gb',
              status: 'Up',
              size: 0,
              extensions: ['pgmq'],
            }}
          />
          <IconButton iconName="activity" />
        </div>
      </Main>
    </>
  );
};

export default Home;
