// use std::sync::Arc;

use liboxen::api;
use liboxen::command;
use liboxen::constants;
use liboxen::constants::DEFAULT_BRANCH_NAME;
use liboxen::error::OxenError;
use liboxen::test;
use liboxen::util;

use futures::future;
// use tokio::sync::Notify;

#[tokio::test]
async fn test_command_push_one_commit() -> Result<(), OxenError> {
    test::run_training_data_repo_test_no_commits_async(|repo| async {
        let mut repo = repo;

        // Track the file
        let train_dir = repo.path.join("train");
        let num_files = util::fs::rcount_files_in_dir(&train_dir);
        command::add(&repo, &train_dir)?;
        // Commit the train dir
        let commit = command::commit(&repo, "Adding training data")?;

        // Set the proper remote
        let remote = test::repo_remote_url_from(&repo.dirname());
        command::config::set_remote(&mut repo, constants::DEFAULT_REMOTE_NAME, &remote)?;

        // Create the repo
        let remote_repo = test::create_remote_repo(&repo).await?;

        // Push it real good
        command::push(&repo).await?;

        let page_num = 1;
        let page_size = num_files + 10;
        let entries =
            api::remote::dir::list(&remote_repo, &commit.id, "train", page_num, page_size).await?;
        assert_eq!(entries.total_entries, num_files);
        assert_eq!(entries.entries.len(), num_files);

        api::remote::repositories::delete(&remote_repo).await?;

        future::ok::<(), OxenError>(()).await
    })
    .await
}

#[tokio::test]
async fn test_command_push_one_commit_check_is_synced() -> Result<(), OxenError> {
    test::run_training_data_repo_test_no_commits_async(|repo| async {
        let mut repo = repo;

        // Track the train and annotations dir
        let train_dir = repo.path.join("train");
        let annotations_dir = repo.path.join("annotations");

        command::add(&repo, &train_dir)?;
        command::add(&repo, &annotations_dir)?;
        // Commit the train dir
        let commit = command::commit(&repo, "Adding training data")?;

        // Set the proper remote
        let remote = test::repo_remote_url_from(&repo.dirname());
        command::config::set_remote(&mut repo, constants::DEFAULT_REMOTE_NAME, &remote)?;

        // Create the repo
        let remote_repo = test::create_remote_repo(&repo).await?;

        // Push it real good
        command::push(&repo).await?;

        // Sleep so it can unpack...
        std::thread::sleep(std::time::Duration::from_secs(2));

        let is_synced = api::remote::commits::commit_is_synced(&remote_repo, &commit.id)
            .await?
            .unwrap();
        assert!(is_synced.is_valid);

        api::remote::repositories::delete(&remote_repo).await?;

        future::ok::<(), OxenError>(()).await
    })
    .await
}

#[tokio::test]
async fn test_command_push_multiple_commit_check_is_synced() -> Result<(), OxenError> {
    test::run_training_data_repo_test_no_commits_async(|repo| async {
        let mut repo = repo;

        // Track the train and annotations dir
        let train_dir = repo.path.join("train");
        let train_bounding_box = repo
            .path
            .join("annotations")
            .join("train")
            .join("bounding_box.csv");

        command::add(&repo, &train_dir)?;
        command::add(&repo, &train_bounding_box)?;
        // Commit the train dir
        command::commit(&repo, "Adding training data")?;

        // Set the proper remote
        let remote = test::repo_remote_url_from(&repo.dirname());
        command::config::set_remote(&mut repo, constants::DEFAULT_REMOTE_NAME, &remote)?;

        // Create the repo
        let remote_repo = test::create_remote_repo(&repo).await?;

        // Push it real good
        command::push(&repo).await?;

        // Sleep so it can unpack...
        std::thread::sleep(std::time::Duration::from_secs(2));

        // Add and commit the rest of the annotations
        // The nlp annotations have duplicates which broke the system at a time
        let annotations_dir = repo.path.join("nlp");
        command::add(&repo, &annotations_dir)?;
        let commit = command::commit(&repo, "adding the rest of the annotations")?;

        // Push again
        command::push(&repo).await?;

        let is_synced = api::remote::commits::commit_is_synced(&remote_repo, &commit.id)
            .await?
            .unwrap();
        assert!(is_synced.is_valid);

        api::remote::repositories::delete(&remote_repo).await?;

        future::ok::<(), OxenError>(()).await
    })
    .await
}

#[tokio::test]
async fn test_command_push_inbetween_two_commits() -> Result<(), OxenError> {
    test::run_training_data_repo_test_no_commits_async(|repo| async {
        let mut repo = repo;
        // Track the train dir
        let train_dir = repo.path.join("train");
        let num_train_files = util::fs::rcount_files_in_dir(&train_dir);
        command::add(&repo, &train_dir)?;
        // Commit the train dur
        command::commit(&repo, "Adding training data")?;

        // Set the proper remote
        let remote = test::repo_remote_url_from(&repo.dirname());
        command::config::set_remote(&mut repo, constants::DEFAULT_REMOTE_NAME, &remote)?;

        // Create the remote repo
        let remote_repo = test::create_remote_repo(&repo).await?;

        // Push the files
        command::push(&repo).await?;

        // Track the test dir
        let test_dir = repo.path.join("test");
        let num_test_files = util::fs::count_files_in_dir(&test_dir);
        command::add(&repo, &test_dir)?;
        let commit = command::commit(&repo, "Adding test data")?;

        // Push the files
        command::push(&repo).await?;

        let page_num = 1;
        let page_size = num_train_files + num_test_files + 5;
        let train_entries =
            api::remote::dir::list(&remote_repo, &commit.id, "/train", page_num, page_size).await?;
        let test_entries =
            api::remote::dir::list(&remote_repo, &commit.id, "/test", page_num, page_size).await?;
        assert_eq!(
            train_entries.total_entries + test_entries.total_entries,
            num_train_files + num_test_files
        );
        assert_eq!(
            train_entries.entries.len() + test_entries.entries.len(),
            num_train_files + num_test_files
        );

        api::remote::repositories::delete(&remote_repo).await?;

        future::ok::<(), OxenError>(()).await
    })
    .await
}

#[tokio::test]
async fn test_command_push_after_two_commits() -> Result<(), OxenError> {
    test::run_training_data_repo_test_no_commits_async(|repo| async {
        // Make mutable copy so we can set remote
        let mut repo = repo;

        // Track the train dir
        let train_dir = repo.path.join("train");
        command::add(&repo, &train_dir)?;
        // Commit the train dur
        command::commit(&repo, "Adding training data")?;

        // Track the test dir
        let test_dir = repo.path.join("test");
        let num_test_files = util::fs::rcount_files_in_dir(&test_dir);
        command::add(&repo, &test_dir)?;
        let commit = command::commit(&repo, "Adding test data")?;

        // Set the proper remote
        let remote = test::repo_remote_url_from(&repo.dirname());
        command::config::set_remote(&mut repo, constants::DEFAULT_REMOTE_NAME, &remote)?;

        // Create the remote repo
        let remote_repo = test::create_remote_repo(&repo).await?;

        // Push the files
        command::push(&repo).await?;

        let page_num = 1;
        let entries = api::remote::dir::list(&remote_repo, &commit.id, ".", page_num, 10).await?;
        assert_eq!(entries.total_entries, 2);
        assert_eq!(entries.entries.len(), 2);

        let page_size = num_test_files + 10;
        let entries =
            api::remote::dir::list(&remote_repo, &commit.id, "test", page_num, page_size).await?;
        assert_eq!(entries.total_entries, num_test_files);
        assert_eq!(entries.entries.len(), num_test_files);

        api::remote::repositories::delete(&remote_repo).await?;

        future::ok::<(), OxenError>(()).await
    })
    .await
}

// This broke when you tried to add the "." directory to add everything, after already committing the train directory.
#[tokio::test]
async fn test_command_push_after_two_commits_adding_dot() -> Result<(), OxenError> {
    test::run_training_data_repo_test_no_commits_async(|repo| async {
        // Make mutable copy so we can set remote
        let mut repo = repo;

        // Track the train dir
        let train_dir = repo.path.join("train");

        command::add(&repo, &train_dir)?;
        // Commit the train dur
        command::commit(&repo, "Adding training data")?;

        // Track the rest of the files
        let full_dir = &repo.path;
        let num_files = util::fs::count_items_in_dir(full_dir);
        command::add(&repo, full_dir)?;
        let commit = command::commit(&repo, "Adding rest of data")?;

        // Set the proper remote
        let remote = test::repo_remote_url_from(&repo.dirname());
        command::config::set_remote(&mut repo, constants::DEFAULT_REMOTE_NAME, &remote)?;

        // Create the remote repo
        let remote_repo = test::create_remote_repo(&repo).await?;

        // Push the files
        command::push(&repo).await?;

        let page_num = 1;
        let page_size = num_files + 10;
        let entries =
            api::remote::dir::list(&remote_repo, &commit.id, ".", page_num, page_size).await?;
        assert_eq!(entries.total_entries, num_files);
        assert_eq!(entries.entries.len(), num_files);

        api::remote::repositories::delete(&remote_repo).await?;

        future::ok::<(), OxenError>(()).await
    })
    .await
}

#[tokio::test]
async fn test_cannot_push_if_remote_not_set() -> Result<(), OxenError> {
    test::run_training_data_repo_test_no_commits_async(|repo| async move {
        // Track the file
        let train_dirname = "train";
        let train_dir = repo.path.join(train_dirname);
        command::add(&repo, &train_dir)?;
        // Commit the train dir
        command::commit(&repo, "Adding training data")?;

        // Should not be able to push
        let result = command::push(&repo).await;
        assert!(result.is_err());
        Ok(())
    })
    .await
}

#[tokio::test]
async fn test_push_branch_with_with_no_new_commits() -> Result<(), OxenError> {
    test::run_training_data_repo_test_no_commits_async(|mut repo| async move {
        // Track a dir
        let train_path = repo.path.join("train");
        command::add(&repo, &train_path)?;
        command::commit(&repo, "Adding train dir")?;

        // Set the proper remote
        let remote = test::repo_remote_url_from(&repo.dirname());
        command::config::set_remote(&mut repo, constants::DEFAULT_REMOTE_NAME, &remote)?;

        // Create Remote
        let remote_repo = test::create_remote_repo(&repo).await?;

        // Push it
        command::push(&repo).await?;

        let new_branch_name = "my-branch";
        api::local::branches::create_checkout(&repo, new_branch_name)?;

        // Push new branch, without any new commits, should still create the branch
        command::push_remote_branch(&repo, constants::DEFAULT_REMOTE_NAME, new_branch_name).await?;

        let remote_branches = api::remote::branches::list(&remote_repo).await?;
        assert_eq!(2, remote_branches.len());

        api::remote::repositories::delete(&remote_repo).await?;

        Ok(())
    })
    .await
}

#[tokio::test]
async fn test_cannot_push_two_separate_empty_roots() -> Result<(), OxenError> {
    test::run_no_commit_remote_repo_test(|remote_repo| async move {
        let ret_repo = remote_repo.clone();

        // Clone the first repo
        test::run_empty_dir_test_async(|first_repo_dir| async move {
            let first_cloned_repo =
                command::clone_url(&remote_repo.remote.url, &first_repo_dir.join("first_repo"))
                    .await?;

            // Clone the second repo
            test::run_empty_dir_test_async(|second_repo_dir| async move {
                let second_cloned_repo = command::clone_url(
                    &remote_repo.remote.url,
                    &second_repo_dir.join("second_repo"),
                )
                .await?;

                // Add to the first repo, after we have the second repo cloned
                let new_file = "new_file.txt";
                let new_file_path = first_cloned_repo.path.join(new_file);
                let new_file_path = test::write_txt_file_to_path(new_file_path, "new file")?;
                command::add(&first_cloned_repo, &new_file_path)?;
                command::commit(&first_cloned_repo, "Adding first file path.")?;
                command::push(&first_cloned_repo).await?;

                // The push to the second version of the same repo should fail
                // Adding two commits to have a longer history that also should fail
                let new_file = "new_file_2.txt";
                let new_file_path = second_cloned_repo.path.join(new_file);
                let new_file_path = test::write_txt_file_to_path(new_file_path, "new file 2")?;
                command::add(&second_cloned_repo, &new_file_path)?;
                command::commit(&second_cloned_repo, "Adding second file path.")?;

                let new_file = "new_file_3.txt";
                let new_file_path = second_cloned_repo.path.join(new_file);
                let new_file_path = test::write_txt_file_to_path(new_file_path, "new file 3")?;
                command::add(&second_cloned_repo, &new_file_path)?;
                command::commit(&second_cloned_repo, "Adding third file path.")?;

                // Push should FAIL
                let result = command::push(&second_cloned_repo).await;
                assert!(result.is_err());

                Ok(second_repo_dir)
            })
            .await?;

            Ok(first_repo_dir)
        })
        .await?;

        Ok(ret_repo)
    })
    .await
}

// Test that we cannot push two completely separate local repos to the same history
// 1) Create repo A with data
// 2) Create repo B with data
// 3) Push Repo A
// 4) Push repo B to repo A and fail
#[tokio::test]
async fn test_cannot_push_two_separate_repos() -> Result<(), OxenError> {
    test::run_training_data_repo_test_fully_committed_async(|mut repo_1| async move {
        test::run_training_data_repo_test_fully_committed_async(|mut repo_2| async move {
            // Add to the first repo
            let new_file = "new_file.txt";
            let new_file_path = repo_1.path.join(new_file);
            let new_file_path = test::write_txt_file_to_path(new_file_path, "new file")?;
            command::add(&repo_1, &new_file_path)?;
            command::commit(&repo_1, "Adding first file path.")?;
            // Set/create the proper remote
            let remote = test::repo_remote_url_from(&repo_1.dirname());
            command::config::set_remote(&mut repo_1, constants::DEFAULT_REMOTE_NAME, &remote)?;
            test::create_remote_repo(&repo_1).await?;
            command::push(&repo_1).await?;

            // Adding two commits to have a longer history that also should fail
            let new_file = "new_file_2.txt";
            let new_file_path = repo_2.path.join(new_file);
            let new_file_path = test::write_txt_file_to_path(new_file_path, "new file 2")?;
            command::add(&repo_2, &new_file_path)?;
            command::commit(&repo_2, "Adding second file path.")?;

            let new_file = "new_file_3.txt";
            let new_file_path = repo_2.path.join(new_file);
            let new_file_path = test::write_txt_file_to_path(new_file_path, "new file 3")?;
            command::add(&repo_2, &new_file_path)?;
            command::commit(&repo_2, "Adding third file path.")?;

            // Set remote to the same as the first repo
            command::config::set_remote(&mut repo_2, constants::DEFAULT_REMOTE_NAME, &remote)?;

            // Push should FAIL
            let result = command::push(&repo_2).await;
            assert!(result.is_err());

            Ok(())
        })
        .await?;

        Ok(())
    })
    .await
}

#[tokio::test]
async fn test_push_many_commits_default_branch() -> Result<(), OxenError> {
    test::run_many_local_commits_empty_sync_remote_test(|local_repo, remote_repo| async move {
        // Current local head
        let local_head = api::local::commits::head_commit(&local_repo)?;

        // Branch name

        // Nothing should be synced on remote and no commit objects created except root
        let history =
            api::remote::commits::list_commit_history(&remote_repo, DEFAULT_BRANCH_NAME).await?;
        assert_eq!(history.len(), 1);

        // Push all to remote
        command::push(&local_repo).await?;

        // Should now have 25 commits on remote
        let history =
            api::remote::commits::list_commit_history(&remote_repo, DEFAULT_BRANCH_NAME).await?;
        assert_eq!(history.len(), 25);

        // Latest commit synced should be == local head, with no unsynced commits
        let sync_response =
            api::remote::commits::latest_commit_synced(&remote_repo, &local_head.id).await?;
        assert_eq!(sync_response.num_unsynced, 0);

        Ok(remote_repo)
    })
    .await
}

#[tokio::test]
async fn test_push_many_commits_new_branch() -> Result<(), OxenError> {
    test::run_many_local_commits_empty_sync_remote_test(|local_repo, remote_repo| async move {
        // Current local head
        let local_head = api::local::commits::head_commit(&local_repo)?;

        // Nothing should be synced on remote and no commit objects created except root
        let history =
            api::remote::commits::list_commit_history(&remote_repo, DEFAULT_BRANCH_NAME).await?;
        assert_eq!(history.len(), 1);

        // Create new local branch
        let new_branch_name = "my-branch";
        api::local::branches::create_checkout(&local_repo, new_branch_name)?;

        // New commit
        let new_file = "new_file.txt";
        let new_file_path = local_repo.path.join(new_file);
        let new_file_path = test::write_txt_file_to_path(new_file_path, "new file")?;
        command::add(&local_repo, &new_file_path)?;
        command::commit(&local_repo, "Adding first file path.")?;

        // Push new branch to remote without first syncing main
        command::push_remote_branch(&local_repo, constants::DEFAULT_REMOTE_NAME, new_branch_name)
            .await?;

        // Should now have 26 commits on remote on new branch, 1 on main
        let history_new =
            api::remote::commits::list_commit_history(&remote_repo, new_branch_name).await?;
        let history_main =
            api::remote::commits::list_commit_history(&remote_repo, DEFAULT_BRANCH_NAME).await?;

        assert_eq!(history_new.len(), 26);
        assert_eq!(history_main.len(), 1);

        // Back to main
        command::checkout(&local_repo, DEFAULT_BRANCH_NAME).await?;

        // Push to remote
        command::push(&local_repo).await?;

        // 25 on main
        let history_main =
            api::remote::commits::list_commit_history(&remote_repo, DEFAULT_BRANCH_NAME).await?;
        assert_eq!(history_main.len(), 25);

        // 0 unsynced on main
        let sync_response =
            api::remote::commits::latest_commit_synced(&remote_repo, &local_head.id).await?;
        assert_eq!(sync_response.num_unsynced, 0);

        Ok(remote_repo)
    })
    .await
}

#[tokio::test]
async fn test_cannot_push_while_another_user_is_pushing() -> Result<(), OxenError> {
    test::run_no_commit_remote_repo_test(|remote_repo| async move {
        let ret_repo = remote_repo.clone();

        // Clone the first repo
        test::run_empty_dir_test_async(|first_repo_dir| async move {
            let first_cloned_repo =
                command::clone_url(&remote_repo.remote.url, &first_repo_dir.join("first_repo"))
                    .await?;

            // Clone the second repo
            test::run_empty_dir_test_async(|second_repo_dir| async move {
                let second_cloned_repo = command::clone_url(
                    &remote_repo.remote.url,
                    &second_repo_dir.join("second_repo"),
                )
                .await?;

                // Add to the first repo, after we have the second repo cloned
                let new_file = "new_file.txt";
                let new_file_path = first_cloned_repo.path.join(new_file);
                let new_file_path = test::write_txt_file_to_path(new_file_path, "new file")?;
                command::add(&first_cloned_repo, &new_file_path)?;
                command::commit(&first_cloned_repo, "Adding first file path.")?;
                command::push(&first_cloned_repo).await?;

                // The push to the second version of the same repo should fail
                // Adding two commits to have a longer history that also should fail
                let new_file = "new_file_2.txt";
                let new_file_path = second_cloned_repo.path.join(new_file);
                let new_file_path = test::write_txt_file_to_path(new_file_path, "new file 2")?;
                command::add(&second_cloned_repo, &new_file_path)?;
                command::commit(&second_cloned_repo, "Adding second file path.")?;

                let new_file = "new_file_3.txt";
                let new_file_path = second_cloned_repo.path.join(new_file);
                let new_file_path = test::write_txt_file_to_path(new_file_path, "new file 3")?;
                command::add(&second_cloned_repo, &new_file_path)?;
                command::commit(&second_cloned_repo, "Adding third file path.")?;

                // Push should FAIL
                let result = command::push(&second_cloned_repo).await;
                assert!(result.is_err());

                Ok(second_repo_dir)
            })
            .await?;

            Ok(first_repo_dir)
        })
        .await?;

        Ok(ret_repo)
    })
    .await
}

// #[tokio::test]
// async fn test_push_cannot_push_while_another_is_pushing() -> Result<(), OxenError> {
//     // IF THIS IS FAILING, it could be a race condition w/ Push A finishing before Push B.
//     // This shouldn't happen unless we _dramatically_ increase our push speed, but try increasing
//     // the number of commits pushed to A, or simulate a push as unlock -> sleep -> lock
//     // Push the Remote Repo
//     test::run_empty_sync_repo_test(|_, remote_repo| async move {
//         let remote_repo_copy = remote_repo.clone();

//         // Clone Repo to User A
//         test::run_empty_dir_test_async(|user_a_repo_dir| async move {
//             let user_a_repo_dir_copy = user_a_repo_dir.clone();
//             let user_a_repo =
//                 command::clone_url(&remote_repo.remote.url, &user_a_repo_dir).await?;

//             // Clone Repo to User B
//             test::run_empty_dir_test_async(|user_b_repo_dir| async move {
//                 let user_b_repo_dir_copy = user_b_repo_dir.clone();
//                 let user_b_repo =
//                     command::clone_url(&remote_repo.remote.url, &user_b_repo_dir).await?;

//                 for i in 1..=10 {
//                     let file_name = format!("file_{}.txt", i);
//                     let file_content = format!("File {}", i);

//                     let file_path = user_a_repo.path.join(&file_name);

//                     // Writing the text file
//                     test::write_txt_file_to_path(file_path.clone(), &file_content)?;

//                     // Adding the file
//                     command::add(&user_a_repo, file_path.clone())?;

//                     // Committing the file
//                     let commit_message = format!("Adding {}", file_name);
//                     command::commit(&user_a_repo, &commit_message)?;
//                 }

//                 // Add file_3 to user B repo
//                 let file_3 = "file_3.txt";
//                 test::write_txt_file_to_path(user_b_repo.path.join(file_3), "File 3")?;
//                 command::add(&user_b_repo, user_b_repo.path.join(file_3))?;
//                 command::commit(&user_b_repo, "Adding file_3")?;

//                 let a_is_done = Arc::new(Notify::new());
//                 let a_is_done_cloned = a_is_done.clone();

//                 // Run async in the background, but keep going w/ B
//                 tokio::spawn(async move {
//                     command::push(&user_a_repo).await.unwrap();
//                     // Let the test thread know we're done
//                     a_is_done_cloned.notify_one();
//                 });

//                 // Wait a bit to make sure the push is in progress (gives A time to acquire the lock)
//                 tokio::time::sleep(std::time::Duration::from_secs(1)).await;

//                 println!("About to push b");
//                 let b_result = command::push(&user_b_repo).await;
//                 println!("Pushed B");
//                 // This should fail due to the lock
//                 assert!(b_result.is_err());

//                 // Wait until the task pushing user_a completes
//                 a_is_done.notified().await;

//                 // Lock should be dropped, pull should now succeed
//                 command::pull(&user_b_repo).await?;

//                 // Can now push
//                 command::push(&user_b_repo).await?;

//                 Ok(user_b_repo_dir_copy)
//             })
//             .await?;

//             Ok(user_a_repo_dir_copy)
//         })
//         .await?;

//         Ok(remote_repo_copy)
//     })
//     .await
// }
