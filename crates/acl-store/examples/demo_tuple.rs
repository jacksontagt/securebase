use acl_engine::TupleStore;
use acl_model::tuple::{ObjectRef, SubjectRef, Tuple};
use acl_store::PostgresTupleStore;
use sqlx::PgPool;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    let pool = PgPool::connect(&url).await.expect("connect");
    let store = PostgresTupleStore::new(pool.clone());

    // Build document:readme#viewer@user:alice
    let obj = ObjectRef::new("document", "readme").unwrap();
    let subj_obj = ObjectRef::new("user", "alice").unwrap();
    let subj = SubjectRef::user(subj_obj, None).unwrap();
    let tuple = Tuple::new(obj, "viewer", subj).unwrap();

    // Write via TupleStore
    store.write(vec![tuple], vec![]).await.unwrap();
    println!("Written: document:readme#viewer@user:alice");

    // Read back directly from DB so we can see the raw row
    let row: (String, String, String, String, String, String) = sqlx::query_as(
        "SELECT object_namespace, object_id, relation,
                subject_namespace, subject_id, subject_relation
         FROM acl.tuples
         WHERE object_namespace='document' AND object_id='readme'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    println!(
        "\nRow in acl.tuples:\n  object_namespace = {:?}\n  object_id        = {:?}\n  relation         = {:?}\n  subject_namespace= {:?}\n  subject_id       = {:?}\n  subject_relation = {:?}  ← empty string = direct user",
        row.0, row.1, row.2, row.3, row.4, row.5
    );

    // Delete the tuple
    let obj2 = ObjectRef::new("document", "readme").unwrap();
    let subj_obj2 = ObjectRef::new("user", "alice").unwrap();
    let subj2 = SubjectRef::user(subj_obj2, None).unwrap();
    let tuple2 = Tuple::new(obj2, "viewer", subj2).unwrap();
    store.write(vec![], vec![tuple2]).await.unwrap();
    println!("\nDeleted. Table is clean.");
}
