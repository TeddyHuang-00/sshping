CLIFF_OPTS := "--tag-pattern '^v[0-9]+.[0-9]+.[0-9]+$'"

# Pre-release to github only
[working-directory: '.']
pre-release LEVEL: test
    cargo release {{LEVEL}} -x

# Release
[working-directory: '.']
release: test
    #!/usr/bin/bash
    VERSION=$(git cliff {{CLIFF_OPTS}} --bump --bumped-version)
    cargo release "${VERSION#v}" 
    echo "Will bump version to $VERSION"
    read -p "Continue? [y/N] " yn;
        if [ "$yn" = "y" ]; then
            echo "Bumping version...";
        else
            echo "Aborting.";
            exit 1;
        fi
    git cliff {{CLIFF_OPTS}} --bump -o CHANGELOG.md
    git add CHANGELOG.md
    git commit -m "chore: update changelog"
    cargo release "${VERSION#v}" -x
