use liboxen::api;
use liboxen::command;
use liboxen::constants;
use liboxen::error::OxenError;
use liboxen::model::StagedEntryStatus;
use liboxen::test;
use liboxen::util;

#[test]
fn test_command_init() -> Result<(), OxenError> {
    test::run_empty_dir_test(|repo_dir| {
        // Init repo
        let repo = command::init(repo_dir)?;

        // Init should create the .oxen directory
        let hidden_dir = util::fs::oxen_hidden_dir(repo_dir);
        let config_file = util::fs::config_filepath(repo_dir);
        assert!(hidden_dir.exists());
        assert!(config_file.exists());

        // Name and id will be random but should be populated
        assert!(!repo.id.is_empty());
        assert!(!repo.name.is_empty());

        // We make an initial parent commit and branch called "main"
        // just to make our lives easier down the line
        let orig_branch = command::current_branch(&repo)?.unwrap();
        assert_eq!(orig_branch.name, constants::DEFAULT_BRANCH_NAME);
        assert!(!orig_branch.commit_id.is_empty());

        Ok(())
    })
}

#[test]
fn test_command_status_empty() -> Result<(), OxenError> {
    test::run_empty_local_repo_test(|repo| {
        let repo_status = command::status(&repo)?;

        assert_eq!(repo_status.added_dirs.len(), 0);
        assert_eq!(repo_status.added_files.len(), 0);
        assert_eq!(repo_status.untracked_files.len(), 0);
        assert_eq!(repo_status.untracked_dirs.len(), 0);

        Ok(())
    })
}

#[test]
fn test_command_commit_nothing_staged() -> Result<(), OxenError> {
    test::run_empty_local_repo_test(|repo| {
        let commits = command::log(&repo)?;
        let initial_len = commits.len();
        command::commit(&repo, "Should not work")?;
        // We should not have added any commits
        assert_eq!(commits.len(), initial_len);
        Ok(())
    })
}

#[test]
fn test_command_status_has_txt_file() -> Result<(), OxenError> {
    test::run_empty_local_repo_test(|repo| {
        // Write to file
        let hello_file = repo.path.join("hello.txt");
        util::fs::write_to_path(&hello_file, "Hello World");

        // Get status
        let repo_status = command::status(&repo)?;
        assert_eq!(repo_status.added_dirs.len(), 0);
        assert_eq!(repo_status.added_files.len(), 0);
        assert_eq!(repo_status.untracked_files.len(), 1);
        assert_eq!(repo_status.untracked_dirs.len(), 0);

        Ok(())
    })
}

#[test]
fn test_command_add_file() -> Result<(), OxenError> {
    test::run_empty_local_repo_test(|repo| {
        // Write to file
        let hello_file = repo.path.join("hello.txt");
        util::fs::write_to_path(&hello_file, "Hello World");

        // Track the file
        command::add(&repo, &hello_file)?;
        // Get status and make sure it is removed from the untracked, and added to the tracked
        let repo_status = command::status(&repo)?;
        assert_eq!(repo_status.added_dirs.len(), 0);
        assert_eq!(repo_status.added_files.len(), 1);
        assert_eq!(repo_status.untracked_files.len(), 0);
        assert_eq!(repo_status.untracked_dirs.len(), 0);

        Ok(())
    })
}

#[test]
fn test_command_commit_file() -> Result<(), OxenError> {
    test::run_empty_local_repo_test(|repo| {
        // Write to file
        let hello_file = repo.path.join("hello.txt");
        util::fs::write_to_path(&hello_file, "Hello World");

        // Track the file
        command::add(&repo, &hello_file)?;
        // Commit the file
        command::commit(&repo, "My message")?;

        // Get status and make sure it is removed from the untracked and added
        let repo_status = command::status(&repo)?;
        assert_eq!(repo_status.added_dirs.len(), 0);
        assert_eq!(repo_status.added_files.len(), 0);
        assert_eq!(repo_status.untracked_files.len(), 0);
        assert_eq!(repo_status.untracked_dirs.len(), 0);

        let commits = command::log(&repo)?;
        assert_eq!(commits.len(), 2);

        Ok(())
    })
}

#[test]
fn test_command_checkout_non_existant_commit_id() -> Result<(), OxenError> {
    test::run_empty_local_repo_test(|repo| {
        // This shouldn't work
        let checkout_result = command::checkout(&repo, "non-existant");
        assert!(checkout_result.is_err());

        Ok(())
    })
}

#[test]
fn test_command_checkout_commit_id() -> Result<(), OxenError> {
    test::run_empty_local_repo_test(|repo| {
        // Write to file
        let hello_file = repo.path.join("hello.txt");
        util::fs::write_to_path(&hello_file, "Hello");
        let world_file = repo.path.join("world.txt");
        util::fs::write_to_path(&world_file, "World");

        // Track the hello file
        command::add(&repo, &hello_file)?;
        // Commit the hello file
        let first_commit = command::commit(&repo, "Adding hello")?;
        assert!(first_commit.is_some());

        // Track the world file
        command::add(&repo, &world_file)?;

        // Commit the world file
        let second_commit = command::commit(&repo, "Adding world")?;
        assert!(second_commit.is_some());

        // We have the world file
        assert!(world_file.exists());

        // We checkout the previous commit
        command::checkout(&repo, &first_commit.unwrap().id)?;

        // Then we do not have the world file anymore
        assert!(!world_file.exists());

        // Check status
        let status = command::status(&repo)?;
        assert!(status.is_clean());

        Ok(())
    })
}

#[test]
fn test_command_commit_dir() -> Result<(), OxenError> {
    test::run_training_data_repo_test_no_commits(|repo| {
        // Track the file
        let train_dir = repo.path.join("train");
        command::add(&repo, &train_dir)?;
        // Commit the file
        command::commit(&repo, "Adding training data")?;

        let repo_status = command::status(&repo)?;
        assert_eq!(repo_status.added_dirs.len(), 0);
        assert_eq!(repo_status.added_files.len(), 0);
        assert_eq!(repo_status.untracked_files.len(), 2);
        assert_eq!(repo_status.untracked_dirs.len(), 2);

        let commits = command::log(&repo)?;
        assert_eq!(commits.len(), 2);

        Ok(())
    })
}

#[test]
fn test_command_commit_dir_recursive() -> Result<(), OxenError> {
    test::run_training_data_repo_test_no_commits(|repo| {
        // Track the annotations dir, which has sub dirs
        let annotations_dir = repo.path.join("annotations");
        command::add(&repo, &annotations_dir)?;
        command::commit(&repo, "Adding annotations data dir, which has two levels")?;

        let repo_status = command::status(&repo)?;
        assert_eq!(repo_status.added_dirs.len(), 0);
        assert_eq!(repo_status.added_files.len(), 0);
        assert_eq!(repo_status.untracked_files.len(), 2);
        assert_eq!(repo_status.untracked_dirs.len(), 2);

        let commits = command::log(&repo)?;
        assert_eq!(commits.len(), 2);

        Ok(())
    })
}

#[test]
fn test_command_checkout_current_branch_name_does_nothing() -> Result<(), OxenError> {
    test::run_empty_local_repo_test(|repo| {
        // Write the first file
        let hello_file = repo.path.join("hello.txt");
        util::fs::write_to_path(&hello_file, "Hello");

        // Track & commit the file
        command::add(&repo, &hello_file)?;
        command::commit(&repo, "Added hello.txt")?;

        // Create and checkout branch
        let branch_name = "feature/world-explorer";
        command::create_checkout_branch(&repo, branch_name)?;
        command::checkout(&repo, branch_name)?;

        Ok(())
    })
}

#[test]
fn test_command_checkout_added_file() -> Result<(), OxenError> {
    test::run_empty_local_repo_test(|repo| {
        // Write the first file
        let hello_file = repo.path.join("hello.txt");
        util::fs::write_to_path(&hello_file, "Hello");

        // Track & commit the file
        command::add(&repo, &hello_file)?;
        command::commit(&repo, "Added hello.txt")?;

        // Get the original branch name
        let orig_branch = command::current_branch(&repo)?.unwrap();

        // Create and checkout branch
        let branch_name = "feature/world-explorer";
        command::create_checkout_branch(&repo, branch_name)?;

        // Write a second file
        let world_file = repo.path.join("world.txt");
        util::fs::write_to_path(&world_file, "World");

        // Track & commit the second file in the branch
        command::add(&repo, &world_file)?;
        command::commit(&repo, "Added world.txt")?;

        // Make sure we have both commits after the initial
        let commits = command::log(&repo)?;
        assert_eq!(commits.len(), 3);

        let branches = command::list_branches(&repo)?;
        assert_eq!(branches.len(), 2);

        // Make sure we have both files on disk in our repo dir
        assert!(hello_file.exists());
        assert!(world_file.exists());

        // Go back to the main branch
        command::checkout(&repo, &orig_branch.name)?;

        // The world file should no longer be there
        assert!(hello_file.exists());
        assert!(!world_file.exists());

        // Go back to the world branch
        command::checkout(&repo, branch_name)?;
        assert!(hello_file.exists());
        assert!(world_file.exists());

        Ok(())
    })
}

#[test]
fn test_command_checkout_added_file_keep_untracked() -> Result<(), OxenError> {
    test::run_empty_local_repo_test(|repo| {
        // Write the first file
        let hello_file = repo.path.join("hello.txt");
        util::fs::write_to_path(&hello_file, "Hello");

        // Have another file lying around we will not remove
        let keep_file = repo.path.join("keep_me.txt");
        util::fs::write_to_path(&keep_file, "I am untracked, don't remove me");

        // Track & commit the file
        command::add(&repo, &hello_file)?;
        command::commit(&repo, "Added hello.txt")?;

        // Get the original branch name
        let orig_branch = command::current_branch(&repo)?.unwrap();

        // Create and checkout branch
        let branch_name = "feature/world-explorer";
        command::create_checkout_branch(&repo, branch_name)?;

        // Write a second file
        let world_file = repo.path.join("world.txt");
        util::fs::write_to_path(&world_file, "World");

        // Track & commit the second file in the branch
        command::add(&repo, &world_file)?;
        command::commit(&repo, "Added world.txt")?;

        // Make sure we have both commits after the initial
        let commits = command::log(&repo)?;
        assert_eq!(commits.len(), 3);

        let branches = command::list_branches(&repo)?;
        assert_eq!(branches.len(), 2);

        // Make sure we have all files on disk in our repo dir
        assert!(hello_file.exists());
        assert!(world_file.exists());
        assert!(keep_file.exists());

        // Go back to the main branch
        command::checkout(&repo, &orig_branch.name)?;

        // The world file should no longer be there
        assert!(hello_file.exists());
        assert!(!world_file.exists());
        assert!(keep_file.exists());

        // Go back to the world branch
        command::checkout(&repo, branch_name)?;
        assert!(hello_file.exists());
        assert!(world_file.exists());
        assert!(keep_file.exists());

        Ok(())
    })
}

#[test]
fn test_command_checkout_modified_file() -> Result<(), OxenError> {
    test::run_empty_local_repo_test(|repo| {
        // Write the first file
        let hello_file = repo.path.join("hello.txt");
        util::fs::write_to_path(&hello_file, "Hello");

        // Track & commit the file
        command::add(&repo, &hello_file)?;
        command::commit(&repo, "Added hello.txt")?;

        // Get the original branch name
        let orig_branch = command::current_branch(&repo)?.unwrap();

        // Create and checkout branch
        let branch_name = "feature/world-explorer";
        command::create_checkout_branch(&repo, branch_name)?;

        // Modify the file
        let hello_file = test::modify_txt_file(hello_file, "World")?;

        // Track & commit the change in the branch
        command::add(&repo, &hello_file)?;
        command::commit(&repo, "Changed file to world")?;

        // It should say World at this point
        assert_eq!(util::fs::read_from_path(&hello_file)?, "World");

        // Go back to the main branch
        command::checkout(&repo, &orig_branch.name)?;

        // The file contents should be Hello, not World
        assert!(hello_file.exists());

        // It should be reverted back to Hello
        assert_eq!(util::fs::read_from_path(&hello_file)?, "Hello");

        Ok(())
    })
}

#[test]
fn test_command_checkout_modified_file_in_subdirectory() -> Result<(), OxenError> {
    test::run_training_data_repo_test_no_commits(|repo| {
        // Get the original branch name
        let orig_branch = command::current_branch(&repo)?.unwrap();

        // Track & commit the file
        let one_shot_path = repo.path.join("annotations/train/one_shot.txt");
        command::add(&repo, &one_shot_path)?;
        command::commit(&repo, "Adding one shot")?;

        // Get OG file contents
        let og_content = util::fs::read_from_path(&one_shot_path)?;

        let branch_name = "feature/change-the-shot";
        command::create_checkout_branch(&repo, branch_name)?;

        let party_ppl_contents = "train/cat_1.jpg 0";
        let one_shot_path = test::modify_txt_file(one_shot_path, party_ppl_contents)?;
        command::add(&repo, &one_shot_path)?;
        command::commit(&repo, "Changing one shot")?;

        // checkout OG and make sure it reverts
        command::checkout(&repo, &orig_branch.name)?;
        let updated_content = util::fs::read_from_path(&one_shot_path)?;
        assert_eq!(og_content, updated_content);

        // checkout branch again and make sure it reverts
        command::checkout(&repo, branch_name)?;
        let updated_content = util::fs::read_from_path(&one_shot_path)?;
        assert_eq!(party_ppl_contents, updated_content);

        Ok(())
    })
}

#[test]
fn test_command_commit_top_level_dir_then_revert() -> Result<(), OxenError> {
    test::run_training_data_repo_test_no_commits(|repo| {
        // Get the original branch name
        let orig_branch = command::current_branch(&repo)?.unwrap();

        // Create a branch to make the changes
        let branch_name = "feature/adding-train";
        command::create_checkout_branch(&repo, branch_name)?;

        // Track & commit (train dir already created in helper)
        let train_path = repo.path.join("train");
        let og_num_files = util::fs::rcount_files_in_dir(&train_path);

        // Add directory
        command::add(&repo, &train_path)?;
        // Make sure we can get the status
        let status = command::status(&repo)?;
        assert_eq!(status.added_dirs.len(), 1);

        // Commit changes
        command::commit(&repo, "Adding train dir")?;
        // Make sure we can get the status and they are no longer added
        let status = command::status(&repo)?;
        assert_eq!(status.added_dirs.len(), 0);

        // checkout OG and make sure it removes the train dir
        command::checkout(&repo, &orig_branch.name)?;
        assert!(!train_path.exists());

        // checkout branch again and make sure it reverts
        command::checkout(&repo, branch_name)?;
        assert!(train_path.exists());
        assert_eq!(util::fs::rcount_files_in_dir(&train_path), og_num_files);

        Ok(())
    })
}

#[test]
fn test_command_add_second_level_dir_then_revert() -> Result<(), OxenError> {
    test::run_training_data_repo_test_no_commits(|repo| {
        // Get the original branch name
        let orig_branch = command::current_branch(&repo)?.unwrap();

        // Create a branch to make the changes
        let branch_name = "feature/adding-annotations";
        command::create_checkout_branch(&repo, branch_name)?;

        // Track & commit (dir already created in helper)
        let new_dir_path = repo.path.join("annotations").join("train");
        let og_num_files = util::fs::rcount_files_in_dir(&new_dir_path);

        command::add(&repo, &new_dir_path)?;
        command::commit(&repo, "Adding train dir")?;

        // checkout OG and make sure it removes the train dir
        command::checkout(&repo, &orig_branch.name)?;
        assert!(!new_dir_path.exists());

        // checkout branch again and make sure it reverts
        command::checkout(&repo, branch_name)?;
        assert!(new_dir_path.exists());
        assert_eq!(util::fs::rcount_files_in_dir(&new_dir_path), og_num_files);

        Ok(())
    })
}

#[test]
fn test_command_add_removed_file() -> Result<(), OxenError> {
    test::run_training_data_repo_test_no_commits(|repo| {
        // (file already created in helper)
        let file_to_remove = repo.path.join("labels.txt");

        // Commit the file
        command::add(&repo, &file_to_remove)?;
        command::commit(&repo, "Adding labels file")?;

        // Delete the file
        std::fs::remove_file(&file_to_remove)?;

        // We should recognize it as missing now
        let status = command::status(&repo)?;
        assert_eq!(status.removed_files.len(), 1);

        Ok(())
    })
}

#[test]
fn test_command_commit_removed_dir() -> Result<(), OxenError> {
    test::run_training_data_repo_test_no_commits(|repo| {
        // (dir already created in helper)
        let dir_to_remove = repo.path.join("train");
        let og_file_count = util::fs::rcount_files_in_dir(&dir_to_remove);

        command::add(&repo, &dir_to_remove)?;
        command::commit(&repo, "Adding train directory")?;

        // Delete the directory
        std::fs::remove_dir_all(&dir_to_remove)?;

        // Add the deleted dir, so that we can commit the deletion
        command::add(&repo, &dir_to_remove)?;

        // Make sure we have the correct amount of files tagged as removed
        let status = command::status(&repo)?;
        assert_eq!(status.added_files.len(), og_file_count);
        assert_eq!(status.added_files[0].1.status, StagedEntryStatus::Removed);

        // Make sure they don't show up in the status
        assert_eq!(status.removed_files.len(), 0);

        Ok(())
    })
}

#[test]
fn test_command_remove_dir_then_revert() -> Result<(), OxenError> {
    test::run_training_data_repo_test_no_commits(|repo| {
        // Get the original branch name
        let orig_branch = command::current_branch(&repo)?.unwrap();

        // (dir already created in helper)
        let dir_to_remove = repo.path.join("train");
        let og_num_files = util::fs::rcount_files_in_dir(&dir_to_remove);

        // track the dir
        command::add(&repo, &dir_to_remove)?;
        command::commit(&repo, "Adding train dir")?;

        // Create a branch to make the changes
        let branch_name = "feature/removing-train";
        command::create_checkout_branch(&repo, branch_name)?;

        // Delete the directory from disk
        std::fs::remove_dir_all(&dir_to_remove)?;

        // Track the deletion
        command::add(&repo, &dir_to_remove)?;
        command::commit(&repo, "Removing train dir")?;

        // checkout OG and make sure it restores the train dir
        command::checkout(&repo, &orig_branch.name)?;
        assert!(dir_to_remove.exists());
        assert_eq!(util::fs::rcount_files_in_dir(&dir_to_remove), og_num_files);

        // checkout branch again and make sure it reverts
        command::checkout(&repo, branch_name)?;
        assert!(!dir_to_remove.exists());

        Ok(())
    })
}

#[test]
fn test_command_push_one_commit() -> Result<(), OxenError> {
    test::run_training_data_repo_test_no_commits(|repo| {
        let mut repo = repo.clone();

        // Track the file
        let train_dir = repo.path.join("train");
        let num_files = util::fs::rcount_files_in_dir(&train_dir);
        command::add(&repo, &train_dir)?;
        // Commit the train dir
        let commit = command::commit(&repo, "Adding training data")?.unwrap();

        // Set the proper remote
        let remote = api::endpoint::repo_url_from(&repo.name);
        command::set_remote(&mut repo, constants::DEFAULT_ORIGIN_NAME, &remote)?;

        // Push it real good
        command::push(&repo)?;

        let page_num = 1;
        let page_size = num_files;
        let entries = api::remote::entries::list_page(&repo, &commit.id, page_num, page_size)?;
        assert_eq!(entries.total_entries, num_files);
        assert_eq!(entries.entries.len(), num_files);

        Ok(())
    })
}

#[test]
fn test_command_push_inbetween_two_commits() -> Result<(), OxenError> {
    test::run_training_data_repo_test_no_commits(|repo| {
        let mut repo = repo.clone();
        // Track the train dir
        let train_dir = repo.path.join("train");
        let mut num_files = util::fs::rcount_files_in_dir(&train_dir);
        command::add(&repo, &train_dir)?;
        // Commit the train dur
        command::commit(&repo, "Adding training data")?;

        // Set the proper remote
        let remote = api::endpoint::repo_url_from(&repo.name);
        command::set_remote(&mut repo, constants::DEFAULT_ORIGIN_NAME, &remote)?;

        // Push the files
        command::push(&repo)?;

        // Track the test dir
        let test_dir = repo.path.join("test");
        num_files += util::fs::rcount_files_in_dir(&test_dir);
        command::add(&repo, &test_dir)?;
        let commit = command::commit(&repo, "Adding test data")?.unwrap();

        // Push the files
        command::push(&repo)?;

        let page_num = 1;
        let page_size = num_files;
        let entries = api::remote::entries::list_page(&repo, &commit.id, page_num, page_size)?;
        assert_eq!(entries.total_entries, num_files);
        assert_eq!(entries.entries.len(), num_files);

        Ok(())
    })
}

#[test]
fn test_command_push_after_two_commits() -> Result<(), OxenError> {
    test::run_training_data_repo_test_no_commits(|repo| {
        // Make mutable copy so we can set remote
        let mut repo = repo.clone();

        // Track the train dir
        let train_dir = repo.path.join("train");
        let mut num_files = util::fs::rcount_files_in_dir(&train_dir);
        command::add(&repo, &train_dir)?;
        // Commit the train dur
        command::commit(&repo, "Adding training data")?;

        // Track the test dir
        let test_dir = repo.path.join("test");
        num_files += util::fs::rcount_files_in_dir(&test_dir);
        command::add(&repo, &test_dir)?;
        let commit = command::commit(&repo, "Adding test data")?.unwrap();

        // Set the proper remote
        let remote = api::endpoint::repo_url_from(&repo.name);
        command::set_remote(&mut repo, constants::DEFAULT_ORIGIN_NAME, &remote)?;

        // Push the files
        command::push(&repo)?;

        let page_num = 1;
        let page_size = num_files;
        let entries = api::remote::entries::list_page(&repo, &commit.id, page_num, page_size)?;
        assert_eq!(entries.total_entries, num_files);
        assert_eq!(entries.entries.len(), num_files);

        Ok(())
    })
}


#[test]
fn test_cannot_push_if_remote_not_set() -> Result<(), OxenError> {
    test::run_training_data_repo_test_no_commits(|repo| {
        // Track the file
        let train_dirname = "train";
        let train_dir = repo.path.join(train_dirname);
        command::add(&repo, &train_dir)?;
        // Commit the train dir
        command::commit(&repo, "Adding training data")?.unwrap();

        // Should not be able to push
        let result = command::push(&repo);
        assert!(result.is_err());
        Ok(())
    })
}

#[test]
fn test_command_push_clone() -> Result<(), OxenError> {
    test::run_training_data_repo_test_no_commits(|mut repo| {
        // Track the file
        let train_dirname = "train";
        let train_dir = repo.path.join(train_dirname);
        let og_num_files = util::fs::rcount_files_in_dir(&train_dir);
        command::add(&repo, &train_dir)?;
        // Commit the train dir
        command::commit(&repo, "Adding training data")?.unwrap();

        // Set the proper remote
        let remote = api::endpoint::repo_url_from(&repo.name);
        command::set_remote(&mut repo, constants::DEFAULT_ORIGIN_NAME, &remote)?;

        // Push it real good
        let remote_repo = command::push(&repo)?;

        // Add a new file
        let party_ppl_filename = "party_ppl.txt";
        let party_ppl_contents = String::from("Wassup Party Ppl");
        let party_ppl_file_path = repo.path.join(party_ppl_filename);
        util::fs::write_to_path(&party_ppl_file_path, &party_ppl_contents);

        // Add and commit and push
        command::add(&repo, &party_ppl_file_path)?;
        let latest_commit = command::commit(&repo, "Adding party_ppl.txt")?.unwrap();
        command::push(&repo)?;

        // run another test with a new repo dir that we are going to sync to
        test::run_empty_dir_test(|new_repo_dir| {
            let cloned_repo = command::clone(&remote_repo.url, new_repo_dir)?;
            let oxen_dir = cloned_repo.path.join(".oxen");
            assert!(oxen_dir.exists());
            command::pull(&cloned_repo)?;

            // Make sure we pulled all of the train dir
            let cloned_train_dir = cloned_repo.path.join(train_dirname);
            let cloned_num_files = util::fs::rcount_files_in_dir(&cloned_train_dir);
            assert_eq!(og_num_files, cloned_num_files);

            // Make sure we have the party ppl file from the next commit
            let cloned_party_ppl_path = cloned_repo.path.join(party_ppl_filename);
            assert!(cloned_party_ppl_path.exists());
            let cloned_contents = util::fs::read_from_path(&cloned_party_ppl_path)?;
            assert_eq!(cloned_contents, party_ppl_contents);

            // Make sure that pull updates local HEAD to be correct
            let head = command::head_commit(&cloned_repo)?;
            assert_eq!(head.unwrap().id, latest_commit.id);

            // Make sure we synced all the commits
            let repo_commits = command::log(&repo)?;
            let cloned_commits = command::log(&cloned_repo)?;
            assert_eq!(repo_commits.len(), cloned_commits.len());

            // Make sure we updated the dbs properly
            let status = command::status(&cloned_repo)?;
            assert!(status.is_clean());
            
            // Have this side add a file, and send it back over
            let send_it_back_filename = "send_it_back.txt";
            let send_it_back_contents = String::from("Hello from the other side");
            let send_it_back_file_path = cloned_repo.path.join(send_it_back_filename);
            util::fs::write_to_path(&send_it_back_file_path, &send_it_back_contents);

            // Add and commit and push
            command::add(&cloned_repo, &send_it_back_file_path)?;
            command::commit(&cloned_repo, "Adding send_it_back.txt")?;
            command::push(&cloned_repo)?;

            // Pull back from the OG Repo
            command::pull(&repo)?;
            let pulled_send_it_back_path = repo.path.join(send_it_back_filename);
            assert!(pulled_send_it_back_path.exists());
            let pulled_contents = util::fs::read_from_path(&pulled_send_it_back_path)?;
            assert_eq!(pulled_contents, send_it_back_contents);

            
            // Modify the party ppl contents
            let party_ppl_contents = String::from("Late to the party");
            util::fs::write_to_path(&party_ppl_file_path, &party_ppl_contents);
            command::add(&repo, &party_ppl_file_path)?;
            command::commit(&repo, "Modified party ppl contents")?;
            command::push(&repo)?;

            // Pull the modifications
            command::pull(&cloned_repo)?;
            let pulled_contents = util::fs::read_from_path(&cloned_party_ppl_path)?;
            assert_eq!(pulled_contents, party_ppl_contents);

            // Remove a file, add, commit, push the change
            std::fs::remove_file(&send_it_back_file_path)?;
            command::add(&cloned_repo, &send_it_back_file_path)?;
            command::commit(&cloned_repo, "Removing the send it back file")?;
            command::push(&cloned_repo)?;

            // Pull down the changes and make sure the file is removed
            command::pull(&repo)?;
            let pulled_send_it_back_path = repo.path.join(send_it_back_filename);
            assert!(!pulled_send_it_back_path.exists());

            Ok(())
        })
    })
}