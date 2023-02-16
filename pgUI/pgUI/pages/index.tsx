import type { NextPage } from 'next';
import Head from 'next/head';
import Main from '../components/Main';

const Home: NextPage = () => {
  return (
    <Main>
      <Head>
        <title>CoreDB</title>
        <meta name="description" content="Welcome to CoreDB" />
        <link rel="icon" href="favicon.ico" />
      </Head>
    </Main>
  );
};

export default Home;
