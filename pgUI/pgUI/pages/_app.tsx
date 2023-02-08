import type { AppProps } from 'next/app';
import getConfig from 'next/config';
import React from 'react';
import Head from 'next/head';
import { useRouter } from 'next/router';

import '../styles/globals.scss';

function WorkspaceChecker(props: React.PropsWithChildren) {
  const { children } = props;

  /** end delete */
  return <>{children}</>;
}

function WithGlobalLoader({
  children,
}: {
  children: React.ReactNode;
}): JSX.Element {
  const router = useRouter();
  const [ready, setReady] = React.useState(false);

  React.useEffect(() => {
    setReady(true);
  }, []);

  if (ready && router.isReady) return <>{children}</>;

  return <></>;
}


function MyApp({ Component, pageProps }: AppProps) {
  console.log('component', Component, 'page props', pageProps);
\
  return (
      <WithGlobalLoader>
        <Head>
          <link rel="icon" href="favicon.ico" />
        </Head>
          <WorkspaceChecker>
            <Component {...pageProps} />
          </WorkspaceChecker>
      </WithGlobalLoader>

  );
}

export default MyApp;
