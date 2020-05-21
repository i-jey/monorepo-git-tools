function make_temp_repo() {
    cd $BATS_TMPDIR
    mkdir -p $1
    cd $1
    if [[ ! -d .git ]]; then
        git init
        git config --local user.email "temp"
        git config --local user.name "temp"
        echo "name of repo: $1" > $1.txt
        git add $1.txt
        git commit -m "initial commit for $1"
    fi
}

function setup() {
    source $BATS_TEST_DIRNAME/../lib/helpers.bsc
    make_temp_repo test_remote_repo
    make_temp_repo test_remote_repo2
    cd $BATS_TMPDIR/test_remote_repo
}

function teardown() {
    cd $BATS_TMPDIR
    if [[ -d test_remote_repo ]]; then
        rm -rf test_remote_repo
    fi
    if [[ -d test_remote_repo2 ]]; then
        rm -rf test_remote_repo2
    fi
}


@test 'can split in a remote github repo via username and repo_name' {
    # https://github.com/nikita-skobov/github-actions-tutorial
    repo_file_contents="
    repo_name=\"github-actions-tutorial\"
    username=\"nikita-skobov\"
    include_as=(
        \"this/path/will/be/created/\" \"\"
    )
    "

    echo "$repo_file_contents" > repo_file.sh

    # a directory called this should not exist at first
    [[ ! -d this ]]

    run $BATS_TEST_DIRNAME/git-split in repo_file.sh
    # now it should exist:
    [[ -d this ]]

    # and just to be safe, check that the whole path to the files
    # is created:
    [[ -d this/path/will/be/created ]]
    [[ -f this/path/will/be/created/LICENSE ]]
    [[ -f this/path/will/be/created/README.md ]]
}

@test 'can split in a remote github repo via username and repo_name from a specific remote_branch' {
    # https://github.com/nikita-skobov/github-actions-tutorial
    repo_file_contents="
    repo_name=\"github-actions-tutorial\"
    remote_branch=\"test-branch\"
    username=\"nikita-skobov\"
    include_as=(
        \"this/path/will/be/created/\" \"\"
    )
    "

    echo "$repo_file_contents" > repo_file.sh

    # a directory called this should not exist at first
    [[ ! -d this ]]

    run $BATS_TEST_DIRNAME/git-split in repo_file.sh
    echo "$output"
    # now it should exist:
    [[ -d this ]]

    # and just to be safe, check that the whole path to the files
    # is created:
    [[ -d this/path/will/be/created ]]
    [[ -f this/path/will/be/created/LICENSE ]]
    [[ -f this/path/will/be/created/README.md ]]
    # this file only exists in the test-branch:
    [[ -f this/path/will/be/created/test-branch-file.txt ]]
}

@test 'can split in a local branch' {
    repo_file_contents="
    repo_name=\"doesnt_matter\"
    include_as=(
        \"this/path/will/be/created/\" \"\"
    )
    "

    echo "$repo_file_contents" > repo_file.sh

    # a directory called this should not exist at first
    [[ ! -d this ]]

    git checkout -b tmp1
    mkdir -p lib
    echo "libfiletext" > lib/file.txt
    git add lib/
    git commit -m "lib commit"
    git checkout master

    run $BATS_TEST_DIRNAME/git-split in repo_file.sh --merge-branch tmp1
    # now it should exist:
    [[ -d this ]]

    # and just to be safe, check that the whole path to the files
    # is created:
    [[ -d this/path/will/be/created ]]
    [[ -f this/path/will/be/created/lib/file.txt ]]
}

@test 'can split in a remote_repo uri' {
    repo_file_contents="
    repo_name=\"doesnt_matter\"
    remote_repo=\"$BATS_TMPDIR/test_remote_repo2\"
    include_as=(
        \"this/path/will/be/created/\" \"\"
    )
    "

    echo "$repo_file_contents" > repo_file.sh

    # a directory called this should not exist at first
    [[ ! -d this ]]

    run $BATS_TEST_DIRNAME/git-split in repo_file.sh
    # now it should exist:
    [[ -d this ]]

    # and just to be safe, check that the whole path to the files
    # is created:
    [[ -d this/path/will/be/created ]]
    [[ -f this/path/will/be/created/test_remote_repo2.txt ]]
}

@test 'can split in a remote_repo with a specific remote_branch' {
    repo_file_contents="
    repo_name=\"doesnt_matter\"
    remote_repo=\"$BATS_TMPDIR/test_remote_repo2\"
    remote_branch=\"test-branch\"
    include_as=(
        \"this/path/will/be/created/\" \"\"
    )
    "

    echo "$repo_file_contents" > repo_file.sh

    # a directory called this should not exist at first
    [[ ! -d this ]]

    cd $BATS_TMPDIR/test_remote_repo2
    git checkout -b test-branch
    mkdir -p lib
    echo "libfiletext" > lib/test-branch-file.txt
    git add lib/
    git commit -m "lib commit"
    git checkout master
    cd -

    run $BATS_TEST_DIRNAME/git-split in repo_file.sh
    # now it should exist:
    [[ -d this ]]

    # and just to be safe, check that the whole path to the files
    # is created:
    [[ -d this/path/will/be/created ]]
    [[ -f this/path/will/be/created/test_remote_repo2.txt ]]
    [[ -f this/path/will/be/created/lib/test-branch-file.txt ]]
}


@test 'can specify an output branch with --output-branch' {
    repo_file_contents="
    repo_name=\"doesnt_matter\"
    remote_repo=\"$BATS_TMPDIR/test_remote_repo2\"
    include_as=(
        \"this/path/will/be/created/\" \"\"
    )
    "

    echo "$repo_file_contents" > repo_file.sh

    # branch should not exist at first:
    run branch_exists some-output-branch
    echo "$output"
    [[ $status -eq 1 ]]

    run $BATS_TEST_DIRNAME/git-split in repo_file.sh --output-branch some-output-branch
    
    # now it should:
    run branch_exists some-output-branch
    [[ $status -eq 0 ]]
    # also we should be on that branch:
    run get_current_branch_name
    [[ $output == "some-output-branch" ]]
}

@test 'can specify an output branch with -o' {
    repo_file_contents="
    repo_name=\"doesnt_matter\"
    remote_repo=\"$BATS_TMPDIR/test_remote_repo2\"
    include_as=(
        \"this/path/will/be/created/\" \"\"
    )
    "

    echo "$repo_file_contents" > repo_file.sh

    # branch should not exist at first:
    run branch_exists some-output-branch
    echo "$output"
    [[ $status -eq 1 ]]

    run $BATS_TEST_DIRNAME/git-split in repo_file.sh -o some-output-branch
    
    # now it should:
    run branch_exists some-output-branch
    [[ $status -eq 0 ]]
    # also we should be on that branch:
    run get_current_branch_name
    [[ $output == "some-output-branch" ]]
}
