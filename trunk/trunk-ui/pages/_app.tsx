import { ClerkProvider, ClerkLoaded } from "@clerk/nextjs";
import type { AppProps } from "next/app";

function MyApp({ Component, pageProps }: AppProps) {
  return (
    <ClerkProvider {...pageProps}>
      <ClerkLoaded>
        <Component {...pageProps} />
      </ClerkLoaded>
    </ClerkProvider>
  );
}

export default MyApp;
