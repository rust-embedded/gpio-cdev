# This script takes care of testing your crate

set -ex

main() {
    cross build --target $TARGET --all-features
    cross build --target $TARGET --release --all-features

    if [ ! -z $DISABLE_TESTS ]; then
        return
    fi

    cross test --target $TARGET --all-features
    cross test --target $TARGET --release --all-features

    # No main binary, so skip the 'cross run' portion
    # cross run --target $TARGET
    # cross run --target $TARGET --release
}

# we don't run the "test phase" when doing deploys
if [ -z $TRAVIS_TAG ]; then
    main
fi
