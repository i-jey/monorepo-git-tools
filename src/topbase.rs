use clap::ArgMatches;
use std::collections::HashSet;

use super::git_helpers;
use super::exec_helpers;
use super::split::Runner;
use super::check_updates::topbase_check_alg;
use super::commands::TOPBASE_CMD_BASE;
use super::commands::TOPBASE_CMD_TOP;
use super::commands::VERBOSE_ARG;
use super::commands::DRY_RUN_ARG;

pub trait Topbase {
    fn topbase(self) -> Self;
}

impl<'a> Topbase for Runner<'a> {
    fn topbase(mut self) -> Self {
        let repo = match self.repo {
            Some(ref r) => r,
            None => panic!("failed to get repo?"),
        };

        // for split commands, we always use current ref,
        // but for topbase command, we check if user provided a top branch
        // if user provided one, we use that, otherwise we use current
        let current_branch = if let Some(ref b) = self.topbase_top_ref {
            b.clone()
        } else {
            match git_helpers::get_current_ref(repo) {
                Some(s) => s,
                None => {
                    println!("Failed to get current branch. not going to rebase");
                    return self;
                },
            }
        };

        // upstream is base
        let upstream_branch = match self.repo_original_ref {
            Some(ref branch) => branch.clone(),
            None => {
                println!("Failed to get repo original ref. Not going to rebase");
                return self;
            },
        };

        let all_upstream_blobs = get_all_blobs_in_branch(upstream_branch.as_str());
        let all_commits_of_current = match git_helpers::get_all_commits_from_ref(repo, current_branch.as_str()) {
            Ok(v) => v,
            Err(e) => panic!("Failed to get all commits! {}", e),
        };

        let num_commits_of_current = all_commits_of_current.len();
        let mut num_commits_to_take = 0;
        let mut rebase_data = vec![];
        let mut cb = |c: &git2::Commit| {
            num_commits_to_take += 1;
            let rebase_interactive_entry = format!("pick {} {}\n", c.id(), c.summary().unwrap());
            rebase_data.push(rebase_interactive_entry);
        };
        topbase_check_alg(all_commits_of_current, all_upstream_blobs, &mut cb);

        // need to reverse it because git rebase interactive
        // takes commits in order of oldest to newest, but
        // we parsed them from newest to oldest
        rebase_data.reverse();

        // we just want to use the actual branch names, not the ref name
        let current_branch = current_branch.replace("refs/heads/", "");
        let upstream_branch = upstream_branch.replace("refs/heads/", "");

        // log the special cases
        if num_commits_to_take == 0 {
            // if we have found that the most recent commit of current_branch already exists
            // on the upstream branch, we should just rebase normally (so that the branch can be fast-forwardable)
            // instead of rebasing interactively
            println!("{}most recent commit of {} exists in {}. rebasing non-interactively", self.log_p, current_branch, upstream_branch);
        } else if num_commits_to_take == num_commits_of_current {
            // if we are trying to topbase on a branch that hasnt been rebased yet,
            // we dont need to topbase, and instead we need to do a regular rebase
            println!("{}no commit of {} exists in {}. rebasing non-interactively", self.log_p, current_branch, upstream_branch);
        }

        let args = match num_commits_to_take {
            // if there's nothing to topbase, then we want to just
            // rebase the last commit onto the upstream branch.
            // this will allow our current branch to be fast-forwardable
            // onto upstream (well really its going to be the exact same branch)
            0 => {
                // if current branch only has one commit, dont use the <branch>~1
                // git rebase syntax. it will cause git rebase to fail
                let rebase_last_one = match num_commits_of_current > 1 {
                    true => "~1",
                    false => "",
                };
                let last_commit_arg = format!("{}{}", current_branch, rebase_last_one);
                let args = vec![
                    "git".into(), "rebase".into(), "--onto".into(),
                    upstream_branch.clone(),
                    last_commit_arg,
                    current_branch.clone()
                ];
                args
            },
            // if we need to topbase the entirety of the current branch
            // it will be better to do a regular rebase
            n => {
                if n == num_commits_of_current {
                    let args = vec![
                        "git".into(), "rebase".into(), upstream_branch.clone(),
                    ];
                    args
                } else {
                    vec![]
                }
            },
        };

        // args will have non-zero length only if
        //  - we need to topbase all commits
        //  - we found no commits to topbase
        if args.len() != 0 {
            if self.dry_run {
                let arg_str = args.join(" ");
                println!("{}", arg_str);
                return self;
            }

            let str_args: Vec<&str> = args.iter().map(|f| f.as_str()).collect();
            let err_msg = match exec_helpers::execute(
                &str_args[..]
            ) {
                Err(e) => Some(vec![format!("{}", e)]),
                Ok(o) => {
                    match o.status {
                        0 => None,
                        _ => Some(vec![o.stderr.lines().next().unwrap().to_string()]),
                    }
                },
            };
            if let Some(err) = err_msg {
                self.status = 1;
                let err_details = match self.verbose {
                    true => format!("{}", err.join("\n")),
                    false => "".into(),
                };
                println!("Failed to rebase\n{}", err_details);
            }
            return self;
        }

        if self.dry_run || self.verbose {
            // since we are already on the rebase_from_branch
            // we dont need to specify that in the git command
            // the below command implies: apply rebased changes in
            // the branch we are already on
            println!("rebase_data=\"{}\"", rebase_data.join(""));
            println!("GIT_SEQUENCE_EDITOR=\"echo $rebase_data >\" git rebase -i --onto {} {}~{} {}",
                upstream_branch,
                current_branch,
                num_commits_to_take,
                current_branch,
            );
            if self.dry_run {
                return self;
            }
        }

        // rebase_data="pick <hash> <msg>
        // pick <hash> <msg>
        // pick <hash> <msg>
        // "
        // rebase_command="echo \"$rebase_data\""
        // GIT_SEQUENCE_EDITOR="$rebase_command >" git rebase -i --onto bottom top~3 top
        let upstream_arg = format!("{}~{}", current_branch, num_commits_to_take);
        let args = [
            "git", "rebase", "-i",
            "--onto", upstream_branch.as_str(),
            upstream_arg.as_str(),
            current_branch.as_str(),
        ];
        let rebase_data_str = rebase_data.join("");
        let rebase_data_str = format!("echo \"{}\" >", rebase_data_str);

        let err_msg = match exec_helpers::execute_with_env(
            &args,
            &["GIT_SEQUENCE_EDITOR"],
            &[rebase_data_str.as_str()],
        ) {
            Err(e) => Some(vec![format!("{}", e)]),
            Ok(o) => {
                match o.status {
                    0 => None,
                    _ => Some(vec![o.stderr.lines().next().unwrap().to_string()]),
                }
            },
        };
        if let Some(err) = err_msg {
            self.status = 1;
            let err_details = match self.verbose {
                true => format!("{}", err.join("\n")),
                false => "".into(),
            };
            println!("Failed to rebase\n{}", err_details);
        }
        self
    }
}

pub enum BlobCheckValue {
    TakeNext,
    TakePrev,
}
use BlobCheckValue::*;
pub struct BlobCheck<'a> {
    pub mode_prev: &'a str,
    pub mode_next: &'a str,
    pub blob_prev: &'a str,
    pub blob_next: &'a str,
    pub path: String,
}

pub fn blob_check_callback_default(blob_check: &BlobCheck) -> Option<BlobCheckValue> {
    match blob_check.is_delete_blob() {
        true => Some(TakePrev),
        false => Some(TakeNext),
    }
}

impl<'a> BlobCheck<'a> {
    fn is_delete_blob(&self) -> bool {
        let blob_prev_not_all_zeroes = ! self.blob_prev.chars().all(|c| c == '0');
        let blob_next_all_zeroes = self.blob_next.chars().all(|c| c == '0');
        blob_next_all_zeroes && blob_prev_not_all_zeroes
    }
}

// run a git diff-tree on the commit id, and parse the output
// and for every blob, if callback returns true,
// insert that blob id into the provided blob hash set
pub fn get_all_blobs_from_commit_with_callback(
    commit_id: &str,
    blob_set: &mut HashSet<String>,
    insert_callback: Option<&dyn Fn(&BlobCheck) -> Option<BlobCheckValue>>,
) {
    // the diff filter is VERY important...
    // A (added), M (modified), C (copied), D (deleted)
    // theres a few more..
    let args = [
        "git", "diff-tree", commit_id, "-r", "--root",
        "--diff-filter=AMCD", "--pretty=oneline"
    ];
    match exec_helpers::execute(&args) {
        Err(e) => panic!("Failed to get blobs from commit {} : {}", commit_id, e),
        Ok(out) => {
            if out.status != 0 { panic!("Failed to get blobs from commit {} : {}", commit_id, out.stderr); }
            for l in out.stdout.lines() {
                // lines starting with colons are the lines
                // that contain blob ids
                if ! l.starts_with(':') { continue; }
                let items = l.split_whitespace().collect::<Vec<&str>>();
                // there are technically 6 items from this output:
                // the last item (items[5]) is a path to the file that this blob
                // is for (and the array could have more than 6 if file names
                // have spaces in them)
                let (
                    mode_prev, mode_next,
                    blob_prev, blob_next,
                    diff_type
                ) = (items[0], items[1], items[2], items[3], items[4]);
                // the path of this blob starts at index 5, but we combine the rest
                // in case there are spaces
                let blob_path = items[5..items.len()].join(" ");
                let blob_check = BlobCheck {
                    mode_prev,
                    mode_next,
                    blob_prev,
                    blob_next,
                    path: blob_path,
                };
                // if user provided a callback, ask the user A) if they want to take this
                // blob, and B) which one to take (next or prev)
                // otherwise, use the default way to decide which one to take
                let should_take = match insert_callback {
                    Some(ref which_to_take_callback) => which_to_take_callback(&blob_check),
                    None => blob_check_callback_default(&blob_check),
                };
                if let Some(which) = should_take {
                    match which {
                        TakeNext => blob_set.insert(blob_next.into()),
                        TakePrev => blob_set.insert(blob_prev.into()),
                    };
                }
            }
        }
    };
}

pub fn get_all_blobs_from_commit<'a>(
    commit_id: &str,
    blob_set: &mut HashSet<String>,
) {
    get_all_blobs_from_commit_with_callback(
        commit_id,
        blob_set,
        None,
    );
}

// perform a rev-list of the branch name to get a list of all commits
// then get every single blob from every single commit, and return
// a hash set containing unique blob ids
pub fn get_all_blobs_in_branch(branch_name: &str) -> HashSet<String> {
    // first get all commits from this branch:
    let args = [
        "git", "rev-list", branch_name,
    ];

    // need the stdout to live outside the match so that the vec of strings
    // lives outside the match
    let mut out_stdout = "".into();
    let commit_ids = match exec_helpers::execute(&args) {
        Err(e) => panic!("Failed to get all blobs of {} : {}", branch_name, e),
        Ok(out) => {
            if out.status != 0 { panic!("Failed to get all blobs of {} : {}", branch_name, out.stderr); }
            out_stdout = out.stdout;
            out_stdout.split_whitespace().collect::<Vec<&str>>()
        },
    };

    let mut blob_set = HashSet::new();
    for commit_id in commit_ids.iter() {
        get_all_blobs_from_commit(commit_id, &mut blob_set);
    }

    return blob_set;
}

pub fn run_topbase(matches: &ArgMatches) {
    // should be safe to unwrap because its a required argument
    let base_branch = matches.value_of(TOPBASE_CMD_BASE).unwrap();
    let top_branch = matches.value_of(TOPBASE_CMD_TOP);
    let mut runner = Runner::new(matches);
    // repo_original_ref is used by the other commands (splitout/splitin)
    // but for topbase this is really the base branch
    runner.repo_original_ref = Some(base_branch.into());
    // if user didnt provide top branch, topbase_top_ref stays None
    // and then the runner.topbase will just use the current branch
    if let Some(t) = top_branch {
        runner.topbase_top_ref = Some(t.to_string());
    }

    runner.save_current_dir()
        .get_repository_from_current_dir()
        .topbase();
}
