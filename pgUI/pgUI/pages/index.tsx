import type { NextPage } from 'next';
import Head from 'next/head';
import Main from '../components/Main';
import Header from '../components/Header';
import SearchBar from '../components/SearchBar';

const Home: NextPage = () => {
  return (
    <>
      <Head>
        <title>CoreDB</title>
        <meta name="description" content="Welcome to CoreDB" />
        <link rel="icon" href="favicon.ico" />
      </Head>
      <Header shortName="Fun" />
      <Main>
        <SearchBar placeholder='Search Instances'/>
      </Main>
    </>
  );
};

export default Home;
