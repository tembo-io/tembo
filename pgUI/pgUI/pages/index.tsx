import React from 'react';
import type { NextPage } from 'next';
import Head from 'next/head';
import Main from '../components/Main';
import Header from '../components/Header';
import SearchBar from '../components/SearchBar';
import InstanceCard from '../components/InstanceCard';
import Link from 'next/link';

const Home: NextPage = () => {
  return (
    <>
      <Head>
        <title>CoreDB</title>
        <meta name="description" content="Welcome to CoreDB" />
        <link rel="icon" href="favicon.ico" />
      </Head>
      <Header userName="Rico Suave" />
      <Main hasLeftBar={false}>
        <SearchBar placeholder="Search Instances" />
        <div
          style={{
            display: 'grid',
            gridTemplateColumns: '1fr 1fr 1fr 1fr',
            gap: '24px',
          }}
        >
          <InstanceCard
            dbName="CoreDB"
            id="1"
            paths
            properties={{
              connection: '',
              cpu: 8,
              dbname: 'My Cool Db',
              environment: 'test',
              memory: '16 gb',
              status: 'Up',
              size: 0,
              extensions: ['pgmq'],
            }}
          />
          <InstanceCard
            dbName="CoreDB"
            id="2"
            paths
            properties={{
              connection: '',
              cpu: 1,
              dbname: 'cold-snowflake-13',
              environment: 'test',
              memory: '16 gb',
              status: 'Up',
              size: 0,
              extensions: ['pgmq', 'pgchron'],
            }}
          />
          <InstanceCard
            dbName="CoreDB"
            id="3"
            paths
            properties={{
              connection: '',
              cpu: 1,
              dbname: 'Jeremiah_was_a_bullfrog',
              environment: 'test',
              memory: '16 gb',
              status: 'Up',
              size: 0,
              extensions: ['pgmq'],
            }}
          />
        </div>
      </Main>
    </>
  );
};

export default Home;
