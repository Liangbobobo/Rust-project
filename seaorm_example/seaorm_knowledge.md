# sea-orm

资料:   
https://docs.rs/sea-orm/1.1.17/sea_orm/

## Entity(定义数据库中表的结构-数据库查询的起点)

Entity 是一个结构体，它代表数据库中的一个表。  
Entity 代表数据库中的一个表。它是对数据库表结构、元数据和行为的抽象。您可以将其视为数据库表的"蓝图"或"接口"。它定义了表的名称、包含的列、主键、索引以及与其他表的关系。

Entity 的主要作用包括：
1. 表结构定义：Entity 结构体定义了对应数据库表的名称以及该表的所有列。每个字段通常对应表中的一个列。
2. 数据类型映射：Entity 中的字段类型与数据库列的数据类型进行映射，确保数据在 Rust 和数据库之间正确转换。
3. 查询构建：Entity 提供了构建数据库查询（如 SELECT, INSERT, UPDATE, DELETE）的方法。
4. 关联关系：Entity 可以定义与其他 Entity 之间的关联关系（例如一对一、一对多、多对多），从而简化复杂查询。
5. 模型操作：Entity 允许您创建、读取、更新和删除数据库记录，而无需直接编写 SQL 语句。
在 SeaORM 中，Entity 通常与 Model 结构体一起使用。Entity 负责定义表的元数据和操作，而 Model 结构体则代表表中的一行数据。
### 如何生成？

需要创建一个 Rust 结构体，并使用 #[derive(DeriveEntityModel)] 宏以及 #[sea_orm(table_name = "your_table_name")] 属性来定义它。
```rust
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "posts")] // 映射到数据库的 'posts' 表
pub struct Entity {
    #[sea_orm(primary_key, auto_increment)]
    pub id: i32,
    pub title: String,
    pub content: String,
    pub published_at: DateTimeUtc,
}

// 必须实现 Relation 和 ActiveModelBehavior
impl Relation for Entity {}
impl ActiveModelBehavior for ActiveModel {}
```
### 用途

1. 定义表结构：通过其字段和 #[sea_orm(...)] 属性
2. 发起查询：它是**所有数据库查询的起点**
3. 定义关联关系：在 impl Relation for Entity 块中定义与其他 Entity 的关系
```rust
// 查询所有 Post
let all_posts: Vec<Model> = Entity::find().all(&db).await?;
// 根据 ID 查询特定 Post
let post_by_id: Option<Model> = Entity::find_by_id(1).one(&db).await?;
// 带有条件的查询
let filtered_posts: Vec<Model> = Entity::find()
    .filter(Column::Title.contains("Rust"))
    .all(&db)
    .await?;
```

## Model

在 SeaORM 中，Model 是一个结构体，它代表了数据库表中一行具体的数据的不可变载体。它与 Entity 紧密相关，但职责不同。Model 的主要作用包括：  
1. 数据载体：Model 结构体用于承载从数据库中检索到的数据，或者准备要插入、更新到数据库中的数据。它的字段通常与 Entity 结构体的字段一一对应，但 Model 结构体通常只包含实际的数据字段，而不包含 Entity 所需的元数据或操作方法。
2. 数据操作：当您从数据库中查询数据时，SeaORM 会将结果集映射到 Model 结构体的实例中。同样，当您需要向数据库中插入或更新数据时，您会创建一个 Model 实例，填充其字段，然后通过 Entity 提供的方法将其持久化到数据库。
3. 类型安全：Model 结构体提供了类型安全的数据访问。通过 Rust 的类型系统，您可以在编译时捕获许多与数据类型不匹配相关的错误。
4. 转换为 ActiveModel 进行更新

简而言之，Entity 描述了数据库表的结构和操作，而 Model 则承载了该表中实际的数据记录。Entity 是对表的抽象，而 Model 是对表中行的抽象。
### 如何生成？

Model 结构体是由 #[derive(DeriveEntityModel)] 宏自动生成的。您不需要手动编写 Model 结构体。它会根据 Entity 中定义的字段自动创建。

对于上面定义的 Post Entity，DeriveEntityModel 会自动生成一个大致如下的 Model：
```rust
// 这是由 DeriveEntityModel 宏自动生成的，您不需要手动编写
#[derive(Clone, Debug, PartialEq, FromQueryResult, Eq)]
pub struct Model {
    pub id: i32,
    pub title: String,
    pub content: String,
    pub published_at: DateTimeUtc,
}
```
### 如何使用？

```rust
let post_model: Model = Entity::find_by_id(1).one(&db).await?.unwrap();
println!("Post Title: {}", post_model.title);

//转换为 ActiveModel 进行更新
let post_model: Model = Entity::find_by_id(1).one(&db).await?.unwrap();
let mut active_model: ActiveModel = post_model.into_active_model();
// ... 然后修改 active_model ...
```

## ActiveModel

ActiveModel 代表数据库表中单行数据的可变载体，专门用于数据操作（插入新记录或更新现有记录）。它的每个字段都被 `sea_orm::Set<T>` 枚举包裹，以明确指示哪些字段将被修改或插入。

### 如何生成？

ActiveModel 结构体也是由 #[derive(DeriveEntityModel)] 宏自动生成的。您不需要手动编写 ActiveModel 结构体。它会根据 Entity 中定义的字段自动创建。

对于上面定义的 Post Entity，DeriveEntityModel 会自动生成一个大致如下的 ActiveModel：
```rust
// 这是由 DeriveEntityModel 宏自动生成的，您不需要手动编写
#[derive(Clone, Debug, PartialEq, Default, Eq)]
pub struct ActiveModel {
    pub id: Set<i32>,
    pub title: Set<String>,
    pub content: Set<String>,
    pub published_at: Set<DateTimeUtc>,
}
```
### 如何使用？

ActiveModel 主要用于：  
插入新记录：创建一个 ActiveModel 实例，并使用 Set() 来填充要插入的字段
```rust
use sea_orm::Set;
use chrono::Utc;

let new_post = ActiveModel {
    title: Set("My First Post".to_owned()),
    content: Set("This is the content of my first post.".to_owned()),
    published_at: Set(Utc::now().into()), // 将 chrono::DateTime<Utc> 转换为 DateTimeUtc
    ..Default::default() // 对于自增 ID 等字段，使用默认值
};
let inserted_post: Model = new_post.insert(&db).await?;
println!("Inserted post: {:?}", inserted_post);
```







宏生成的 Model 和 ActiveModel 都是 `struct`。                                                                                                                   它们不是 trait。                                                                                                                                                 * `struct` (结构体)：定义了数据的结构和类型。Model 和 ActiveModel                 都是用来承载数据的。                                                          * `trait` (特性)：定义了行为（方法签名），可以被 struct 实现。                                                                                                 #[derive(DeriveEntityModel)] 和 #[derive(DeriveIntoActiveModel)]                这些宏的作用是：                                                                                                                                                 1. 为你的 `struct` 自动生成代码，使其符合 SeaORM 内部定义的某些 trait 的要求    2. 实现这些 `trait`，从而让你的 Model 和 ActiveModel 拥有 SeaORM                   提供的各种数据库操作方法（例如 Model 可以调用 find()，ActiveModel 可以调用      insert() 或 update()）。                                                                                                                                    所以，你可以把它们理解为：                                                                                                                                       * Model 是一个数据结构（struct），它通过宏实现了 EntityTrait                      等特性，从而具备了查询能力。                                                  * ActiveModel 也是一个数据结构（struct），它通过宏实现了 ActiveModelTrait         等特性，从而具备了插入和更新数据的能力。                                                                                                                     它们本身是数据容器，而宏则赋予了这些容器与数据库交互的“行为”。





















## Entity Model ActiveModel之间关系

在 SeaORM 中，这三个结构体共同构成了一个强大的 ORM 层，它们各自承担不同的职责，但又紧密协作：  
1. Entity：数据库表的抽象定义和操作入口
2. Model：数据库表中单行数据的不可变载体
3. ActiveModel：数据库表中单行数据的可变载体，用于数据操作（插入、更新）