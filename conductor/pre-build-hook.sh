# brings coredb-operator into the build context
# intended to be used as pre-build-hook for the Tembo build github action
# https://github.com/tembo-io/tembo/blob/main/.github/actions/build-and-push-to-quay/action.yml
cp -r ../coredb-operator ./